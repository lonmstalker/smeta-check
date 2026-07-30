//! HTTP-слой auth: маршруты, общие DTO и работа с refresh-cookie.
//! Хендлеры — в подмодулях: session (вход/сессии), recovery (восстановление
//! пароля), totp (2FA), oauth_http (VK/Яндекс).

pub mod account;
pub mod oauth_http;
pub mod recovery;
pub mod session;
pub mod sessions_http;
pub mod totp;
pub mod verify;

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;
use crate::auth::sessions::{self, Session};
use crate::core::config::Settings;
use crate::core::error::ApiError;
use crate::users::User;

/// Логин и код 2FA — это пара коротких строк; мегабайта здесь взяться неоткуда
const AUTH_BODY_LIMIT_BYTES: usize = 16 * 1024;

pub fn router(settings: &Settings) -> Router<AppState> {
    let router = Router::new()
        .route("/api/auth/register", post(session::register))
        .route("/api/auth/verify-email", post(verify::verify_email))
        .route("/api/auth/login", post(session::login))
        .route("/api/auth/2fa/verify", post(session::verify_2fa))
        .route("/api/auth/refresh", post(session::refresh))
        .route("/api/auth/logout", post(session::logout))
        .route("/api/auth/forgot", post(recovery::forgot))
        .route("/api/auth/reset", post(recovery::reset))
        .route("/api/auth/2fa/setup", post(totp::setup))
        .route("/api/auth/2fa/enable", post(totp::enable))
        .route("/api/auth/2fa/disable", post(totp::disable))
        .route("/api/auth/password", post(account::change_password))
        .route("/api/auth/email", post(account::request_email_change))
        .route(
            "/api/auth/email/confirm",
            post(account::confirm_email_change),
        )
        .route(
            "/api/auth/sessions",
            get(sessions_http::list).delete(sessions_http::revoke_others),
        )
        .route(
            "/api/auth/sessions/{id}",
            axum::routing::delete(sessions_http::revoke),
        )
        .route("/api/auth/oauth/{provider}/start", get(oauth_http::start))
        .route(
            "/api/auth/oauth/{provider}/callback",
            get(oauth_http::callback),
        );
    let router =
        same_origin_only(router, settings).layer(DefaultBodyLimit::max(AUTH_BODY_LIMIT_BYTES));
    // Перебор паролей и спам письмами гасим лимитом на все auth-ручки;
    // RATE_LIMIT_AUTH_RPM=0 — выключить (тесты, e2e)
    crate::core::rate_limit::limit_per_ip(
        router,
        settings.rate_limit_auth_rpm,
        settings.trust_proxy,
    )
}

/// Эти маршруты выдают и гасят refresh-cookie, поэтому чужому сайту здесь
/// делать нечего: браузер сам проставляет Origin, и не наш мы отклоняем.
/// Запрос без Origin — не из браузера (curl, мобильный клиент), там подделывать
/// нечего: cookie туда никто автоматически не подставит.
fn same_origin_only<S: Clone + Send + Sync + 'static>(
    router: Router<S>,
    settings: &Settings,
) -> Router<S> {
    let mut allowed = vec![settings.public_url.trim_end_matches('/').to_owned()];
    allowed.extend(settings.cors_origins.iter().cloned());
    let allowed = Arc::new(allowed);
    router.route_layer(axum::middleware::from_fn(
        move |req: Request, next: Next| {
            let allowed = Arc::clone(&allowed);
            async move {
                let origin = req
                    .headers()
                    .get(header::ORIGIN)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned);
                match origin {
                    Some(origin) if !allowed.contains(&origin) => {
                        tracing::warn!(origin, "запрос к auth с чужого origin отклонён");
                        ApiError::forbidden().into_response()
                    }
                    _ => next.run(req).await,
                }
            }
        },
    ))
}

pub(super) const REFRESH_COOKIE: &str = "refresh_token";

/// Короткое описание клиента для списка сессий («Chrome, macOS»).
/// Сырой User-Agent никуда не записывается.
pub(super) fn client_of(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .and_then(sessions::describe_client)
}

pub(super) fn refresh_cookie(
    settings: &Settings,
    value: String,
    max_age_days: i64,
) -> Cookie<'static> {
    // Secure не ставим в dev (http://localhost); в проде включается флагом
    Cookie::build((REFRESH_COOKIE, value))
        .path("/api/auth")
        .http_only(true)
        .secure(settings.cookie_secure)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::days(max_age_days))
        .build()
}

pub(super) fn session_response(
    settings: &Settings,
    jar: CookieJar,
    session: Session,
    user: User,
    status: StatusCode,
) -> Response {
    let jar = jar.add(refresh_cookie(
        settings,
        session.refresh_token,
        sessions::REFRESH_TTL_DAYS,
    ));
    (
        status,
        jar,
        Json(TokenResponse {
            access_token: session.access_token,
            user,
        }),
    )
        .into_response()
}

// --- общие DTO (описаны в OpenAPI; min_length сверяется тестом с константами)

#[derive(Deserialize, ToSchema)]
pub struct Credentials {
    #[schema(format = "email")]
    pub email: String,
    #[schema(min_length = 8)]
    pub password: String,
}

#[derive(Serialize, ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub user: User,
}

#[derive(Serialize, ToSchema)]
pub struct LoginResponse {
    /// присутствует, если 2FA не требуется
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// если true — второй шаг: POST /api/auth/2fa/verify с pending_token
    pub requires_2fa: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_token: Option<String>,
}
