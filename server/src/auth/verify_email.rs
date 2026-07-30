//! Подтверждение email: при регистрации уходит письмо со ссылкой,
//! переход по ссылке ставит отметку на пользователе.
//! Тот же паттерн, что восстановление пароля: в БД — только хеш токена.

use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use super::{hash_token, new_raw_token};
use crate::core::error::ApiError;
use crate::core::{i18n, mailer};
use crate::users::{self, UserRecord};

const VERIFY_TTL_HOURS: i64 = 24;

/// Создать токен и отправить письмо со ссылкой подтверждения
pub async fn send_verification(
    pool: &PgPool,
    base_url: &str,
    user: &UserRecord,
) -> Result<(), ApiError> {
    let raw = new_raw_token();
    sqlx::query(
        "INSERT INTO email_verification_tokens (token_hash, user_id, expires_at)
         VALUES ($1, $2, $3)",
    )
    .bind(hash_token(raw.as_bytes()))
    .bind(user.id)
    .bind(OffsetDateTime::now_utc() + Duration::hours(VERIFY_TTL_HOURS))
    .execute(pool)
    .await?;
    let lang = i18n::current_lang();
    let link = format!("{base_url}/verify-email?token={raw}");
    let body = i18n::translate(
        lang,
        "email-verify-body",
        &[("link", link), ("hours", VERIFY_TTL_HOURS.to_string())],
    );
    let subject = i18n::translate(lang, "email-verify-subject", &[]);
    mailer::send(pool, &user.email, &subject, &body).await?;
    Ok(())
}

/// Отметить почту подтверждённой по одноразовому токену из письма
pub async fn verify_email(pool: &PgPool, raw_token: &str) -> Result<(), ApiError> {
    let row: Option<(uuid::Uuid,)> = sqlx::query_as(
        "UPDATE email_verification_tokens SET used_at = now()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()
         RETURNING user_id",
    )
    .bind(hash_token(raw_token.as_bytes()))
    .fetch_optional(pool)
    .await?;
    let Some((user_id,)) = row else {
        return Err(ApiError::validation("error-invalid-token"));
    };
    users::set_email_verified(pool, user_id).await?;
    Ok(())
}
