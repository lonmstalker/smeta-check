//! Пользователи и роли.

pub mod http;

use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Admin,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Admin => "admin",
        }
    }

    pub fn parse(s: &str) -> Role {
        // значения ограничены CHECK-ограничением в БД
        if s == "admin" {
            Role::Admin
        } else {
            Role::User
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub role: Role,
    /// включён ли второй фактор
    pub totp_enabled: bool,
    /// подтверждена ли почта (по ссылке из письма)
    pub email_verified: bool,
    /// как обращаться к пользователю; не задано — показываем почту
    pub display_name: Option<String>,
    /// язык интерфейса и писем: код из i18n::ALL_LANGS
    pub locale: Option<String>,
}

#[derive(sqlx::FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub role: String,
    pub totp_secret: Option<String>,
    pub email_verified: bool,
    pub display_name: Option<String>,
    pub locale: Option<String>,
}

impl UserRecord {
    pub fn to_user(&self) -> User {
        User {
            id: self.id,
            email: self.email.clone(),
            role: Role::parse(&self.role),
            totp_enabled: self.totp_secret.is_some(),
            email_verified: self.email_verified,
            display_name: self.display_name.clone(),
            locale: self.locale.clone(),
        }
    }
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> sqlx::Result<Option<UserRecord>> {
    sqlx::query_as(
        "SELECT id, email, password_hash, role, totp_secret,
                email_verified_at IS NOT NULL AS email_verified, display_name, locale
         FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

/// executor'ом может быть и пул, и открытая транзакция (ротация сессии)
pub async fn find_by_id(
    executor: impl sqlx::PgExecutor<'_>,
    id: Uuid,
) -> sqlx::Result<Option<UserRecord>> {
    sqlx::query_as(
        "SELECT id, email, password_hash, role, totp_secret,
                email_verified_at IS NOT NULL AS email_verified, display_name, locale
         FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(executor)
    .await
}

pub async fn create(
    pool: &PgPool,
    email: &str,
    password_hash: Option<&str>,
) -> sqlx::Result<UserRecord> {
    sqlx::query_as(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2)
         RETURNING id, email, password_hash, role, totp_secret,
                   email_verified_at IS NOT NULL AS email_verified, display_name, locale",
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await
}

/// Отметить почту подтверждённой; возвращает уже обновлённую запись
pub async fn set_email_verified(pool: &PgPool, id: Uuid) -> sqlx::Result<Option<UserRecord>> {
    sqlx::query_as(
        "UPDATE users SET email_verified_at = now() WHERE id = $1
         RETURNING id, email, password_hash, role, totp_secret,
                   email_verified_at IS NOT NULL AS email_verified, display_name, locale",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn set_password(pool: &PgPool, id: Uuid, hash: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(hash)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Изменить профиль. NULL-параметр = «оставить как было», поэтому запрос
/// PATCH может прислать только то, что действительно меняется.
pub async fn update_profile(
    pool: &PgPool,
    id: Uuid,
    display_name: Option<&str>,
    locale: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE users SET display_name = COALESCE($2::text, display_name),
                          locale = COALESCE($3::text, locale)
         WHERE id = $1",
    )
    .bind(id)
    .bind(display_name)
    .bind(locale)
    .execute(pool)
    .await
    .map(|_| ())
}

/// Сменить адрес почты (после подтверждения нового): почта считается
/// подтверждённой, ведь пользователь только что открыл письмо на ней.
pub async fn set_email(pool: &PgPool, id: Uuid, email: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET email = $2, email_verified_at = now() WHERE id = $1")
        .bind(id)
        .bind(email)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Сменить роль по адресу почты. false — пользователя с таким адресом нет.
/// Роль сидит в access-токене, поэтому увидит её пользователь после
/// перевхода (или через 15 минут, когда токен обновится сам).
pub async fn set_role_by_email(pool: &PgPool, email: &str, role: Role) -> sqlx::Result<bool> {
    let result = sqlx::query("UPDATE users SET role = $2 WHERE email = $1")
        .bind(email.trim().to_ascii_lowercase())
        .bind(role.as_str())
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn set_totp_secret(pool: &PgPool, id: Uuid, secret: Option<&str>) -> sqlx::Result<()> {
    sqlx::query("UPDATE users SET totp_secret = $2 WHERE id = $1")
        .bind(id)
        .bind(secret)
        .execute(pool)
        .await
        .map(|_| ())
}
