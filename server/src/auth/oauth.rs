//! Универсальный вход через OAuth2 (authorization code). Провайдер
//! подключается КОНФИГУРАЦИЕЙ, без кода — см. docs/oauth.md (готовые env
//! для VK ID и Яндекс ID). Схема: start -> редирект к провайдеру ->
//! callback -> обмен кода на токен -> запрос профиля -> вход/регистрация.

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::core::config::{OauthProvider, Settings};
use crate::core::error::ApiError;
use crate::users::{self, UserRecord};

fn not_configured(name: &str) -> ApiError {
    ApiError::validation("error-oauth-not-configured").arg("provider", name)
}

/// Настроенный провайдер или понятная ошибка «вход через X не настроен»
pub fn require_provider<'a>(
    settings: &'a Settings,
    name: &str,
) -> Result<&'a OauthProvider, ApiError> {
    settings
        .oauth
        .get(&name.to_ascii_lowercase())
        .ok_or_else(|| not_configured(name))
}

pub fn authorize_url(cfg: &OauthProvider, redirect_uri: &str, state: &str) -> String {
    // разбираемость auth_url проверена при чтении конфигурации; если она всё
    // же нарушена — уводить пользователя некуда, возвращаем адрес как есть
    let Ok(mut url) = reqwest::Url::parse(&cfg.auth_url) else {
        return cfg.auth_url.clone();
    };
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &cfg.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("scope", &cfg.scope)
        .append_pair("state", state);
    url.to_string()
}

/// Код от провайдера -> наш пользователь (существующий или новый)
pub async fn login_with_code(
    pool: &PgPool,
    cfg: &OauthProvider,
    redirect_uri: &str,
    code: &str,
) -> Result<UserRecord, ApiError> {
    let failed = || ApiError::unauthorized("error-oauth-failed").arg("provider", cfg.name.clone());

    let client = reqwest::Client::new();
    let token_response: Value = client
        .post(&cfg.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", &cfg.client_id),
            ("client_secret", cfg.client_secret.expose()),
            ("redirect_uri", redirect_uri),
        ])
        .send()
        .await
        .map_err(|_| failed())?
        .json()
        .await
        .map_err(|_| failed())?;
    let access_token = token_response
        .pointer("/access_token")
        .and_then(Value::as_str)
        .ok_or_else(failed)?;

    let profile: Value = client
        .get(&cfg.userinfo_url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|_| failed())?
        .json()
        .await
        .map_err(|_| failed())?;

    let provider_user_id = profile
        .pointer(&cfg.id_pointer)
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .ok_or_else(failed)?;
    let email = cfg
        .email_pointer
        .as_ref()
        .and_then(|p| profile.pointer(p))
        .and_then(Value::as_str)
        .map(str::to_owned);

    find_or_create_user(pool, cfg, &provider_user_id, email.as_deref()).await
}

/// Найти пользователя по привязке провайдера или завести нового.
/// Публична ради тестов: настоящий провайдер в тестах не нужен.
pub async fn find_or_create_user(
    pool: &PgPool,
    cfg: &OauthProvider,
    provider_user_id: &str,
    email: Option<&str>,
) -> Result<UserRecord, ApiError> {
    let provider = cfg.name.as_str();
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM identities WHERE provider = $1 AND provider_user_id = $2",
    )
    .bind(provider)
    .bind(provider_user_id)
    .fetch_optional(pool)
    .await?;
    if let Some((user_id,)) = existing {
        return users::find_by_id(pool, user_id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("error-unauthorized"));
    }

    // К чужому аккаунту по email цепляемся, только если провайдер этот адрес
    // проверяет (TRUST_EMAIL=true). Иначе — свой аккаунт с синтетическим
    // адресом: NOT NULL UNIQUE на users.email всё равно требует значения.
    let synthetic = || format!("{provider}.{provider_user_id}@oauth.local");
    let email = match (cfg.trust_email, email) {
        (true, Some(email)) => email.to_ascii_lowercase(),
        _ => synthetic(),
    };
    let user = match users::find_by_email(pool, &email).await? {
        Some(user) => user,
        None => {
            // адрес пришёл от провайдера (или синтетический) — письмом его
            // подтверждать не нужно, сразу считаем подтверждённым
            let created = users::create(pool, &email, None).await?;
            users::set_email_verified(pool, created.id)
                .await?
                .unwrap_or(created)
        }
    };
    sqlx::query("INSERT INTO identities (provider, provider_user_id, user_id) VALUES ($1, $2, $3)")
        .bind(provider)
        .bind(provider_user_id)
        .bind(user.id)
        .execute(pool)
        .await?;
    Ok(user)
}
