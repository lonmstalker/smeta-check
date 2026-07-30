//! Домен аутентификации: регистрация, вход, сессии (refresh-ротация),
//! восстановление пароля, 2FA (TOTP). Все функции транспорто-независимы:
//! их зовёт HTTP-слой, телеграм-бот или тесты напрямую.

pub mod account;
pub mod http;
pub mod jwt;
pub mod oauth;
pub mod password;
pub mod sessions;
pub mod verify_email;

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};
use totp_rs::{Algorithm, TOTP};
use uuid::Uuid;

use crate::core::error::ApiError;
use crate::core::{i18n, mailer};
use crate::users::{self, UserRecord};

const RESET_TTL_MINUTES: i64 = 60;

// --- регистрация и вход ---------------------------------------------------

pub async fn register(pool: &PgPool, email: &str, pw: &str) -> Result<UserRecord, ApiError> {
    let email = normalize_email(email, "email")?;
    validate_password(pw, "password")?;
    if users::find_by_email(pool, &email).await?.is_some() {
        return Err(ApiError::conflict("error-email-taken").field("email"));
    }
    let hash = password::hash(pw).await?;
    // два одновременных запроса на один адрес проходят проверку выше оба;
    // второго ловит UNIQUE в БД — это тот же конфликт, а не сбой сервера
    users::create(pool, &email, Some(&hash))
        .await
        .map_err(|err| match err {
            sqlx::Error::Database(ref db) if db.is_unique_violation() => {
                ApiError::conflict("error-email-taken")
            }
            other => other.into(),
        })
}

/// `field` — имя поля формы, к которому привязать ошибку: в регистрации это
/// «password», в смене пароля — «new_password».
pub(crate) fn validate_password(pw: &str, field: &'static str) -> Result<(), ApiError> {
    if pw.chars().count() < password::MIN_PASSWORD_LEN {
        return Err(ApiError::validation("error-password-short")
            .arg("min", password::MIN_PASSWORD_LEN)
            .field(field));
    }
    Ok(())
}

/// Привести адрес к каноничному виду и отсечь опечатки. Без полноценного
/// разбора RFC: «есть @ и точка после» ловит промахи, а настоящая проверка
/// адреса — только письмо с подтверждением.
pub(crate) fn normalize_email(email: &str, field: &'static str) -> Result<String, ApiError> {
    let email = email.trim().to_ascii_lowercase();
    if !email
        .split_once('@')
        .is_some_and(|(l, r)| !l.is_empty() && r.contains('.'))
    {
        return Err(ApiError::validation("error-validation-email").field(field));
    }
    Ok(email)
}

pub enum LoginOutcome {
    Done(UserRecord),
    /// пароль верен, но включена 2FA — нужен второй шаг
    Requires2fa(Uuid),
}

pub async fn login(pool: &PgPool, email: &str, pw: &str) -> Result<LoginOutcome, ApiError> {
    let email = email.trim().to_ascii_lowercase();
    let invalid = || ApiError::unauthorized("error-invalid-credentials");
    let Some(user) = users::find_by_email(pool, &email).await? else {
        // выравниваем время ответа: хешируем и для несуществующего email,
        // чтобы по скорости ответа нельзя было перебирать базу адресов
        let _ = password::verify(pw, DUMMY_HASH).await;
        return Err(invalid());
    };
    let Some(hash) = &user.password_hash else {
        return Err(invalid());
    };
    if !password::verify(pw, hash).await {
        return Err(invalid());
    }
    if user.totp_secret.is_some() {
        return Ok(LoginOutcome::Requires2fa(user.id));
    }
    Ok(LoginOutcome::Done(user))
}

// валидный argon2-хеш строки "dummy" — только для выравнивания времени
const DUMMY_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$C29tZXNhbHRzb21lc2FsdA$8f2VYysvvUlleFvsdEbyR3s1Cew7yeH3WcQVMDBLnJA";

// --- одноразовые токены (сессии, сброс пароля, подтверждение почты) -------

pub(crate) fn hash_token(raw: &[u8]) -> Vec<u8> {
    Sha256::digest(raw).to_vec()
}

pub(crate) fn new_raw_token() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    hex::encode(bytes)
}

/// Удалить то, что уже никогда не сработает: протухшие и использованные
/// токены домена. Зовётся фоновым воркером (`crate::jobs`); свои таблицы
/// домен чистит сам, снаружи в них никто не лезет.
pub async fn cleanup_expired_tokens(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < now() OR revoked_at IS NOT NULL")
        .execute(pool)
        .await?;
    sqlx::query(
        "DELETE FROM password_reset_tokens WHERE expires_at < now() OR used_at IS NOT NULL",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "DELETE FROM email_verification_tokens WHERE expires_at < now() OR used_at IS NOT NULL",
    )
    .execute(pool)
    .await?;
    Ok(())
}

// --- восстановление пароля -------------------------------------------------

/// Всегда отвечает успехом — не раскрываем, какие адреса есть в базе
pub async fn forgot_password(pool: &PgPool, base_url: &str, email: &str) -> Result<(), ApiError> {
    let email = email.trim().to_ascii_lowercase();
    let Some(user) = users::find_by_email(pool, &email).await? else {
        return Ok(());
    };
    let raw = new_raw_token();
    sqlx::query(
        "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(hash_token(raw.as_bytes()))
    .bind(user.id)
    .bind(OffsetDateTime::now_utc() + Duration::minutes(RESET_TTL_MINUTES))
    .execute(pool)
    .await?;
    let lang = i18n::current_lang();
    let link = format!("{base_url}/reset?token={raw}");
    let body = i18n::translate(
        lang,
        "email-reset-body",
        &[("link", link), ("minutes", RESET_TTL_MINUTES.to_string())],
    );
    let subject = i18n::translate(lang, "email-reset-subject", &[]);
    mailer::send(pool, &email, &subject, &body).await?;
    Ok(())
}

pub async fn reset_password(pool: &PgPool, raw_token: &str, new_pw: &str) -> Result<(), ApiError> {
    validate_password(new_pw, "password")?;
    let row: Option<(Uuid,)> = sqlx::query_as(
        "UPDATE password_reset_tokens SET used_at = now()
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()
         RETURNING user_id",
    )
    .bind(hash_token(raw_token.as_bytes()))
    .fetch_optional(pool)
    .await?;
    let Some((user_id,)) = row else {
        return Err(ApiError::validation("error-invalid-token"));
    };
    users::set_password(pool, user_id, &password::hash(new_pw).await?).await?;
    // новый пароль = все старые сессии недействительны
    sessions::revoke_all(pool, user_id).await?;
    Ok(())
}

// --- 2FA (TOTP) -------------------------------------------------------------

fn totp_for(secret_b32: &str, email: &str) -> Result<TOTP, ApiError> {
    let secret = totp_rs::Secret::Encoded(secret_b32.to_owned())
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("bad totp secret: {e:?}"))?;
    // 6 цифр / 30 секунд — то, чего ждут Google Authenticator и аналоги
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret,
        Some("business-project".into()),
        email.to_owned(),
    )
    .map_err(|e| anyhow::anyhow!("totp build: {e}").into())
}

fn totp_code_valid(secret_b32: &str, email: &str, code: &str) -> Result<bool, ApiError> {
    Ok(totp_for(secret_b32, email)?
        .check_current(code.trim())
        .unwrap_or(false))
}

pub struct TotpSetup {
    pub secret: String,
    pub otpauth_url: String,
}

/// Шаг 1 включения: выдаём секрет; он станет активен после подтверждения кодом
pub async fn totp_setup(pool: &PgPool, user_id: Uuid) -> Result<TotpSetup, ApiError> {
    let user = require_user(pool, user_id).await?;
    if user.totp_secret.is_some() {
        return Err(ApiError::conflict("error-totp-already-enabled"));
    }
    let secret = totp_rs::Secret::generate_secret();
    let secret_b32 = secret.to_encoded().to_string();
    let url = totp_for(&secret_b32, &user.email)?.get_url();
    Ok(TotpSetup {
        secret: secret_b32,
        otpauth_url: url,
    })
}

/// Шаг 2: пользователь ввёл код из приложения — включаем
pub async fn totp_enable(
    pool: &PgPool,
    user_id: Uuid,
    secret_b32: &str,
    code: &str,
) -> Result<(), ApiError> {
    let user = require_user(pool, user_id).await?;
    if user.totp_secret.is_some() {
        return Err(ApiError::conflict("error-totp-already-enabled"));
    }
    if !totp_code_valid(secret_b32, &user.email, code)? {
        return Err(ApiError::validation("error-invalid-totp"));
    }
    users::set_totp_secret(pool, user_id, Some(secret_b32)).await?;
    Ok(())
}

pub async fn totp_disable(pool: &PgPool, user_id: Uuid, code: &str) -> Result<(), ApiError> {
    let user = require_user(pool, user_id).await?;
    let Some(secret) = &user.totp_secret else {
        return Err(ApiError::conflict("error-totp-not-enabled"));
    };
    if !totp_code_valid(secret, &user.email, code)? {
        return Err(ApiError::validation("error-invalid-totp"));
    }
    users::set_totp_secret(pool, user_id, None).await?;
    Ok(())
}

/// Второй шаг входа: проверка кода по pending-токену
pub async fn verify_2fa_login(
    pool: &PgPool,
    user_id: Uuid,
    code: &str,
) -> Result<UserRecord, ApiError> {
    let user = require_user(pool, user_id).await?;
    let Some(secret) = &user.totp_secret else {
        return Err(ApiError::conflict("error-totp-not-enabled"));
    };
    if !totp_code_valid(secret, &user.email, code)? {
        return Err(ApiError::validation("error-invalid-totp"));
    }
    Ok(user)
}

pub(crate) async fn require_user(pool: &PgPool, user_id: Uuid) -> Result<UserRecord, ApiError> {
    users::find_by_id(pool, user_id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))
}
