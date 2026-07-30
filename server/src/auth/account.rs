//! Самообслуживание аккаунта: смена пароля и смена адреса почты.
//! Обе операции требуют текущий пароль — угнанная вкладка не должна уметь
//! отобрать аккаунт целиком.

use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use super::{hash_token, new_raw_token, normalize_email, password, require_user, sessions};
use crate::core::error::ApiError;
use crate::core::{i18n, mailer};
use crate::users;

const EMAIL_CHANGE_TTL_MINUTES: i64 = 60;

/// Сменить пароль. Все прочие сессии гасим: если пароль меняют из-за утечки,
/// чужой вход обязан прекратиться прямо сейчас.
pub async fn change_password(
    pool: &PgPool,
    user_id: Uuid,
    current: &str,
    new: &str,
) -> Result<(), ApiError> {
    verify_current_password(pool, user_id, current).await?;
    super::validate_password(new, "new_password")?;
    users::set_password(pool, user_id, &password::hash(new).await?).await?;
    sessions::revoke_all(pool, user_id).await?;
    Ok(())
}

/// Шаг 1 смены почты: письмо со ссылкой на НОВЫЙ адрес и уведомление на старый.
/// Пока ссылку не открыли, адрес остаётся прежним.
pub async fn request_email_change(
    pool: &PgPool,
    base_url: &str,
    user_id: Uuid,
    new_email: &str,
    current_password: &str,
) -> Result<(), ApiError> {
    let user = verify_current_password(pool, user_id, current_password).await?;
    let new_email = normalize_email(new_email, "new_email")?;
    if new_email == user.email {
        return Err(ApiError::validation("error-email-same").field("new_email"));
    }
    if users::find_by_email(pool, &new_email).await?.is_some() {
        return Err(ApiError::conflict("error-email-taken").field("new_email"));
    }

    let raw = new_raw_token();
    sqlx::query(
        "INSERT INTO email_change_tokens (token_hash, user_id, new_email, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(hash_token(raw.as_bytes()))
    .bind(user_id)
    .bind(&new_email)
    .bind(OffsetDateTime::now_utc() + Duration::minutes(EMAIL_CHANGE_TTL_MINUTES))
    .execute(pool)
    .await?;

    let lang = i18n::current_lang();
    let link = format!("{base_url}/confirm-email?token={raw}");
    let body = i18n::translate(
        lang,
        "email-change-body",
        &[
            ("link", link),
            ("minutes", EMAIL_CHANGE_TTL_MINUTES.to_string()),
        ],
    );
    mailer::send(
        pool,
        &new_email,
        &i18n::translate(lang, "email-change-subject", &[]),
        &body,
    )
    .await?;
    // старый адрес узнаёт о смене, даже если её затеял не хозяин аккаунта
    mailer::send(
        pool,
        &user.email,
        &i18n::translate(lang, "email-change-notice-subject", &[]),
        &i18n::translate(lang, "email-change-notice-body", &[("email", new_email)]),
    )
    .await?;
    Ok(())
}

/// Шаг 2: пользователь открыл ссылку из письма — адрес меняется и сразу
/// считается подтверждённым.
pub async fn confirm_email_change(pool: &PgPool, raw_token: &str) -> Result<(), ApiError> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "UPDATE email_change_tokens SET used_at = now()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()
         RETURNING user_id, new_email",
    )
    .bind(hash_token(raw_token.as_bytes()))
    .fetch_optional(pool)
    .await?;
    let Some((user_id, new_email)) = row else {
        return Err(ApiError::validation("error-invalid-token"));
    };
    // пока письмо ждало, адрес мог занять кто-то другой
    if users::find_by_email(pool, &new_email).await?.is_some() {
        return Err(ApiError::conflict("error-email-taken"));
    }
    // и мог занять прямо между проверкой и записью — уникальный индекс
    // ловит и этот случай, ответ должен быть тем же понятным конфликтом
    users::set_email(pool, user_id, &new_email)
        .await
        .map_err(|err| match err {
            sqlx::Error::Database(ref db) if db.is_unique_violation() => {
                ApiError::conflict("error-email-taken")
            }
            other => other.into(),
        })?;
    Ok(())
}

/// Общая проверка «это точно ты»: без текущего пароля ничего не меняем
async fn verify_current_password(
    pool: &PgPool,
    user_id: Uuid,
    current: &str,
) -> Result<users::UserRecord, ApiError> {
    let user = require_user(pool, user_id).await?;
    let Some(hash) = &user.password_hash else {
        // вошли через VK/Яндекс и пароля не заводили — сначала сброс пароля
        return Err(ApiError::validation("error-no-password").field("current_password"));
    };
    if !password::verify(current, hash).await {
        return Err(ApiError::validation("error-wrong-password").field("current_password"));
    }
    Ok(user)
}
