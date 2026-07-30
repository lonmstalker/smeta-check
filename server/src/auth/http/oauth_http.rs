//! Вход через внешних провайдеров (VK ID, Яндекс ID — конфигом, docs/oauth.md).
//! Это редиректы, а не JSON API, поэтому в OpenAPI эти два маршрута не входят.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;
use sqlx::PgPool;

use super::{client_of, refresh_cookie};
use crate::auth::{jwt, oauth, sessions};
use crate::core::config::Settings;
use crate::core::error::ApiError;

const OAUTH_STATE_COOKIE: &str = "oauth_state";

fn callback_uri(settings: &Settings, provider: &str) -> String {
    format!("{}/api/auth/oauth/{provider}/callback", settings.public_url)
}

pub(crate) async fn start(
    Path(provider): Path<String>,
    State(settings): State<Arc<Settings>>,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let cfg = oauth::require_provider(&settings, &provider)?;
    let state = uuid::Uuid::new_v4().to_string();
    let url = oauth::authorize_url(cfg, &callback_uri(&settings, &provider), &state);
    let jar = jar.add(
        Cookie::build((OAUTH_STATE_COOKIE, state))
            .path("/api/auth/oauth")
            .http_only(true)
            .secure(settings.cookie_secure)
            .same_site(axum_extra::extract::cookie::SameSite::Lax)
            .max_age(time::Duration::minutes(10))
            .build(),
    );
    Ok((jar, Redirect::temporary(&url)).into_response())
}

#[derive(Deserialize)]
pub(crate) struct CallbackQuery {
    code: String,
    state: String,
}

pub(crate) async fn callback(
    Path(provider): Path<String>,
    Query(query): Query<CallbackQuery>,
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let cfg = oauth::require_provider(&settings, &provider)?;
    // state из cookie должен совпасть с state из редиректа — защита от CSRF
    if jar.get(OAUTH_STATE_COOKIE).map(|c| c.value()) != Some(query.state.as_str()) {
        return Err(ApiError::unauthorized("error-oauth-failed").arg("provider", provider));
    }
    let redirect_uri = callback_uri(&settings, &provider);
    let user = oauth::login_with_code(&pool, cfg, &redirect_uri, &query.code).await?;
    let jar = jar.remove(
        Cookie::build((OAUTH_STATE_COOKIE, ""))
            .path("/api/auth/oauth")
            .build(),
    );

    // второй фактор обязателен и здесь: вход через провайдера его не отменяет.
    // pending-токен уезжает во фрагменте — он не попадает ни в логи прокси,
    // ни в Referer, а сессии сам по себе не даёт (нужен код из приложения)
    if user.totp_secret.is_some() {
        let pending = jwt::sign_pending_2fa(&settings.jwt_secret, user.id);
        return Ok((
            jar,
            Redirect::temporary(&format!("/login#pending={pending}")),
        )
            .into_response());
    }

    let session = sessions::issue(
        &pool,
        &settings.jwt_secret,
        &user,
        client_of(&headers).as_deref(),
    )
    .await?;
    // access-токен не передаём через URL; фронт при загрузке сам сделает refresh
    let jar = jar.add(refresh_cookie(
        &settings,
        session.refresh_token,
        sessions::REFRESH_TTL_DAYS,
    ));
    Ok((jar, Redirect::temporary("/")).into_response())
}
