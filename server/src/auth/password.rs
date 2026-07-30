//! Пароли: argon2id с параметрами по умолчанию (рекомендация OWASP).
//!
//! Хеш считается десятки миллисекунд чистого CPU, поэтому наружу торчат
//! только async-функции: работа уходит в spawn_blocking и не держит
//! worker-поток tokio, на котором в это время могли бы жить другие запросы.

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

/// Требование к длине; то же число зафиксировано в OpenAPI-схеме
/// (тест openapi_matches_constants следит, чтобы они не разошлись)
pub const MIN_PASSWORD_LEN: usize = 8;

pub async fn hash(password: &str) -> anyhow::Result<String> {
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || hash_blocking(&password)).await?
}

pub async fn verify(password: &str, stored_hash: &str) -> bool {
    let password = password.to_owned();
    let stored_hash = stored_hash.to_owned();
    tokio::task::spawn_blocking(move || verify_blocking(&password, &stored_hash))
        .await
        .unwrap_or(false)
}

fn hash_blocking(password: &str) -> anyhow::Result<String> {
    let mut salt_bytes = [0u8; 16];
    rand::fill(&mut salt_bytes);
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| anyhow::anyhow!(e))?;
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!(e))?
        .to_string())
}

fn verify_blocking(password: &str, stored_hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
