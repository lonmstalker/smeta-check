//! JWT. Access живёт 15 минут и хранится только в памяти фронта.
//! Отдельное назначение (purpose) у каждого типа токена: access нельзя
//! использовать как pending-2fa и наоборот.
//!
//! Секрет подписи приходит параметром из проверенной конфигурации — ленивого
//! чтения окружения здесь нет, поэтому прод не может стартовать без секрета.

use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::config::Secret;

pub const ACCESS_TTL_SECS: i64 = 15 * 60;
pub const PENDING_2FA_TTL_SECS: i64 = 5 * 60;

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub role: String,
    pub purpose: String,
    pub exp: i64,
}

fn sign(secret: &Secret, sub: Uuid, role: &str, purpose: &str, ttl_secs: i64) -> String {
    let claims = Claims {
        sub,
        role: role.to_owned(),
        purpose: purpose.to_owned(),
        exp: time::OffsetDateTime::now_utc().unix_timestamp() + ttl_secs,
    };
    #[expect(
        clippy::expect_used,
        reason = "hmac-подпись сериализуемых claims не может дать ошибку"
    )]
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("jwt encoding never fails with hmac")
}

/// None = токен просрочен, подделан или не того назначения
fn verify(secret: &Secret, token: &str, purpose: &str) -> Option<Claims> {
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()?;
    (data.claims.purpose == purpose).then_some(data.claims)
}

pub fn sign_access(secret: &Secret, sub: Uuid, role: &str) -> String {
    sign(secret, sub, role, "access", ACCESS_TTL_SECS)
}

pub fn verify_access(secret: &Secret, token: &str) -> Option<Claims> {
    verify(secret, token, "access")
}

/// Промежуточный токен между вводом пароля и вводом кода 2FA
pub fn sign_pending_2fa(secret: &Secret, sub: Uuid) -> String {
    sign(secret, sub, "", "2fa", PENDING_2FA_TTL_SECS)
}

pub fn verify_pending_2fa(secret: &Secret, token: &str) -> Option<Claims> {
    verify(secret, token, "2fa")
}
