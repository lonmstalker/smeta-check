//! Сессии пользователя: выдача пары токенов, ротация refresh, список активных
//! сессий и их отзыв.
//!
//! Refresh-токен ротируется при каждом обновлении: появляется новая строка, а
//! старая гасится — так замеченная кража токена стоит вору доступа. Чтобы
//! пользователь при этом не видел новую «сессию» каждые пятнадцать минут, у
//! сессии есть сквозной session_id и время начала, которые переезжают в новую
//! строку. created_at живой строки = когда сессией пользовались в последний раз.

use serde::Serialize;
use sqlx::PgPool;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{hash_token, new_raw_token};
use crate::auth::jwt;
use crate::core::config::Secret;
use crate::core::error::ApiError;
use crate::users::{self, UserRecord};

pub const REFRESH_TTL_DAYS: i64 = 30;

pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
}

/// Новая сессия после успешного входа
pub async fn issue(
    pool: &PgPool,
    secret: &Secret,
    user: &UserRecord,
    client: Option<&str>,
) -> Result<Session, ApiError> {
    let raw = new_raw_token();
    sqlx::query(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at, client)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user.id)
    .bind(hash_token(raw.as_bytes()))
    .bind(OffsetDateTime::now_utc() + Duration::days(REFRESH_TTL_DAYS))
    .bind(client)
    .execute(pool)
    .await?;
    Ok(Session {
        access_token: jwt::sign_access(secret, user.id, &user.role),
        refresh_token: raw,
    })
}

/// Обновление сессии с ротацией: старый refresh гасится, выдаётся новый —
/// но это та же сессия, поэтому session_id и время начала переезжают.
///
/// Гашение и выдача — в одной транзакции: иначе «выйти на всех устройствах»,
/// пришедшееся ровно между ними, не задело бы новую строку, и украденный
/// токен пережил бы отзыв.
pub async fn refresh(
    pool: &PgPool,
    secret: &Secret,
    raw: &str,
    client: Option<&str>,
) -> Result<(Session, UserRecord), ApiError> {
    let unauthorized = || ApiError::unauthorized("error-unauthorized");
    let mut tx = pool.begin().await?;
    let row: Option<(Uuid, Uuid, OffsetDateTime, Option<String>)> = sqlx::query_as(
        "UPDATE refresh_tokens SET revoked_at = now()
         WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()
         RETURNING user_id, session_id, started_at, client",
    )
    .bind(hash_token(raw.as_bytes()))
    .fetch_optional(&mut *tx)
    .await?;
    let Some((user_id, session_id, started_at, old_client)) = row else {
        return Err(unauthorized());
    };
    let user = users::find_by_id(&mut *tx, user_id)
        .await?
        .ok_or_else(unauthorized)?;

    let new_raw = new_raw_token();
    sqlx::query(
        "INSERT INTO refresh_tokens
             (user_id, token_hash, expires_at, session_id, started_at, client)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user.id)
    .bind(hash_token(new_raw.as_bytes()))
    .bind(OffsetDateTime::now_utc() + Duration::days(REFRESH_TTL_DAYS))
    .bind(session_id)
    .bind(started_at)
    .bind(client.map(str::to_owned).or(old_client))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        Session {
            access_token: jwt::sign_access(secret, user.id, &user.role),
            refresh_token: new_raw,
        },
        user,
    ))
}

pub async fn logout(pool: &PgPool, raw: &str) -> Result<(), ApiError> {
    sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE token_hash = $1")
        .bind(hash_token(raw.as_bytes()))
        .execute(pool)
        .await?;
    Ok(())
}

/// Погасить все сессии пользователя — например, после смены пароля
pub async fn revoke_all(pool: &PgPool, user_id: Uuid) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Сессия глазами пользователя. Время — RFC 3339 в UTC: показывать его в
/// местном формате — работа браузера, а не сервера.
#[derive(Serialize, ToSchema)]
pub struct SessionInfo {
    pub id: Uuid,
    /// когда вошли
    pub created_at: String,
    /// когда сессией пользовались в последний раз
    pub last_seen_at: String,
    /// «Chrome, macOS» — если клиент представился
    pub client: Option<String>,
    /// эта сессия — та, из которой пришёл запрос
    pub current: bool,
}

#[derive(sqlx::FromRow)]
struct SessionRow {
    session_id: Uuid,
    started_at: OffsetDateTime,
    /// время выпуска живой строки = когда сессией пользовались в последний раз
    created_at: OffsetDateTime,
    client: Option<String>,
    token_hash: Vec<u8>,
}

pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    current_raw: Option<&str>,
) -> Result<Vec<SessionInfo>, ApiError> {
    let rows: Vec<SessionRow> = sqlx::query_as(
        "SELECT session_id, started_at, created_at, client, token_hash
         FROM refresh_tokens
         WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let current_hash = current_raw.map(|raw| hash_token(raw.as_bytes()));
    Ok(rows
        .into_iter()
        .map(|row| SessionInfo {
            id: row.session_id,
            created_at: rfc3339(row.started_at),
            last_seen_at: rfc3339(row.created_at),
            client: row.client,
            current: current_hash.as_ref() == Some(&row.token_hash),
        })
        .collect())
}

/// Погасить одну сессию. false — такой активной сессии у пользователя нет.
pub async fn revoke(pool: &PgPool, user_id: Uuid, session_id: Uuid) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now()
         WHERE user_id = $1 AND session_id = $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(session_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// «Выйти на всех остальных устройствах»: текущая сессия остаётся жить.
/// Если предъявленный токен не наш, ничего не гасим и говорим об этом —
/// иначе кнопка молча «срабатывала» бы, не сделав ничего.
pub async fn revoke_others(
    pool: &PgPool,
    user_id: Uuid,
    current_raw: &str,
) -> Result<u64, ApiError> {
    let current: Option<(Uuid,)> = sqlx::query_as(
        "SELECT session_id FROM refresh_tokens
         WHERE token_hash = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(hash_token(current_raw.as_bytes()))
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some((current_session,)) = current else {
        return Err(ApiError::unauthorized("error-unauthorized"));
    };
    let result = sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now()
         WHERE user_id = $1 AND revoked_at IS NULL AND session_id <> $2",
    )
    .bind(user_id)
    .bind(current_session)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_default()
}

/// Короткое описание клиента для списка сессий: «Chrome, macOS».
/// Сырой User-Agent не храним: он точнее, чем нужно, и лежал бы месяцами.
pub fn describe_client(user_agent: &str) -> Option<String> {
    let browser = [
        ("Firefox", "Firefox"),
        ("Edg", "Edge"),
        ("Chrome", "Chrome"),
        ("Safari", "Safari"),
    ]
    .into_iter()
    .find(|(needle, _)| user_agent.contains(needle))
    .map(|(_, name)| name);
    let os = [
        ("Windows", "Windows"),
        ("Android", "Android"),
        ("iPhone", "iOS"),
        ("iPad", "iOS"),
        ("Mac OS", "macOS"),
        ("Linux", "Linux"),
    ]
    .into_iter()
    .find(|(needle, _)| user_agent.contains(needle))
    .map(|(_, name)| name);
    match (browser, os) {
        (Some(browser), Some(os)) => Some(format!("{browser}, {os}")),
        (Some(one), None) | (None, Some(one)) => Some(one.to_owned()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn client_description_is_short_and_readable() {
        let chrome_mac = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/120.0 Safari/537.36";
        assert_eq!(
            super::describe_client(chrome_mac).as_deref(),
            Some("Chrome, macOS")
        );
        // Edge представляется и как Chrome — важно не перепутать
        let edge = "Mozilla/5.0 (Windows NT 10.0) Chrome/120.0 Safari/537.36 Edg/120.0";
        assert_eq!(
            super::describe_client(edge).as_deref(),
            Some("Edge, Windows")
        );
        assert_eq!(super::describe_client("curl/8.4.0"), None);
    }
}
