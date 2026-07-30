pub mod auth;
pub mod core;
pub mod estimates;
pub mod jobs;
pub mod users;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, FromRef};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router, middleware};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::core::config::Settings;

/// Запрос, который не уложился в это время, уже никому не нужен: клиент ушёл,
/// а соединение и поток БД всё ещё заняты. У загрузки файла таймаут свой —
/// поэтому этот слой висит на всех маршрутах, КРОМЕ неё (см. `app`).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Тело больше мегабайта у нас появиться неоткуда: единственный маршрут с
/// файлом — загрузка сметы, и у неё свой потолок
const BODY_LIMIT_BYTES: usize = 1024 * 1024;

/// Логам фронта хватает и этого: ручка открыта без входа
const LOG_BODY_LIMIT_BYTES: usize = 16 * 1024;

/// Общее состояние приложения: пул БД и уже проверенная конфигурация.
/// Хендлеру нужен только пул — он берёт `State<PgPool>`; нужна настройка —
/// `State<Arc<Settings>>`. Этим занимается `FromRef` ниже.
#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub settings: Arc<Settings>,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> PgPool {
        state.pool.clone()
    }
}

impl FromRef<AppState> for Arc<Settings> {
    fn from_ref(state: &AppState) -> Arc<Settings> {
        state.settings.clone()
    }
}

/// Всё HTTP-приложение: домены + сквозные слои (request-id, логи+метрики, язык)
pub fn app(state: AppState) -> Router {
    let settings = state.settings.clone();
    // Всё, кроме загрузки файла: общий потолок тела и общий таймаут. Слои
    // навешиваются здесь, а не на всё приложение, потому что снаружи их уже
    // не снять: вложенный таймаут не может быть длиннее внешнего.
    let common = Router::new()
        .merge(auth::http::router(&settings))
        .merge(users::http::router())
        .merge(estimates::http::router())
        .merge(core::health::router())
        .merge(frontend_log_router(&settings))
        .route(
            "/api/openapi.json",
            get(|| async { Json(ApiDoc::openapi()) }),
        )
        .layer(DefaultBodyLimit::max(BODY_LIMIT_BYTES))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            REQUEST_TIMEOUT,
        ));
    let mut router = common
        .merge(estimates::http::upload_router(&settings))
        .with_state(state)
        .layer(middleware::from_fn(core::i18n::lang_middleware))
        .layer(middleware::from_fn(core::telemetry::track_http))
        .layer(core::telemetry::request_id_layer());
    // SPA раздаётся тем же бинарём, поэтому CORS по умолчанию не нужен;
    // внешнему браузерному клиенту — перечислить его origin'ы в CORS_ORIGINS
    if !settings.cors_origins.is_empty() {
        router = router.layer(cors_layer(&settings.cors_origins));
    }
    router
}

/// CORS для перечисленных origin'ов; с куками, поэтому без wildcard
fn cors_layer(origins: &[String]) -> tower_http::cors::CorsLayer {
    use axum::http::{HeaderValue, Method, header};
    let list: Vec<HeaderValue> = origins.iter().filter_map(|o| o.parse().ok()).collect();
    tower_http::cors::CorsLayer::new()
        .allow_origin(list)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        .allow_credentials(true)
}

/// Ошибки и события фронтенда попадают в общий поток логов
#[derive(Deserialize)]
struct FrontendLog {
    level: String,
    message: String,
    #[serde(default)]
    context: serde_json::Value,
}

/// Ручку зовёт кто угодно, включая гостя с белым экраном, поэтому вход не
/// требуется — от залива логов защищают лимит по IP и потолок размера тела.
fn frontend_log_router<S: Clone + Send + Sync + 'static>(settings: &Settings) -> Router<S> {
    let router = Router::new()
        .route("/api/logs", post(frontend_log))
        .layer(DefaultBodyLimit::max(LOG_BODY_LIMIT_BYTES));
    core::rate_limit::limit_per_ip(router, settings.rate_limit_logs_rpm, settings.trust_proxy)
}

async fn frontend_log(Json(log): Json<FrontendLog>) -> StatusCode {
    let message = log.message.chars().take(2000).collect::<String>();
    let context = log
        .context
        .to_string()
        .chars()
        .take(2000)
        .collect::<String>();
    match log.level.as_str() {
        "error" => tracing::error!(target: "frontend", %message, %context),
        "warn" => tracing::warn!(target: "frontend", %message, %context),
        _ => tracing::info!(target: "frontend", %message, %context),
    }
    StatusCode::ACCEPTED
}

#[derive(OpenApi)]
#[openapi(
    info(title = "smeta-check API", description = "Контракт между бэкендом и клиентами"),
    modifiers(&BearerSecurity),
    components(schemas(core::error::ErrorBody)),
    paths(
        auth::http::session::register,
        auth::http::session::login,
        auth::http::session::verify_2fa,
        auth::http::session::refresh,
        auth::http::session::logout,
        auth::http::recovery::forgot,
        auth::http::recovery::reset,
        auth::http::verify::verify_email,
        auth::http::totp::setup,
        auth::http::totp::enable,
        auth::http::totp::disable,
        auth::http::account::change_password,
        auth::http::account::request_email_change,
        auth::http::account::confirm_email_change,
        auth::http::sessions_http::list,
        auth::http::sessions_http::revoke,
        auth::http::sessions_http::revoke_others,
        core::health::live,
        core::health::ready,
        core::health::version,
        users::http::me,
        users::http::update_me,
        estimates::http::upload_estimate,
        estimates::http::list_estimates,
        estimates::http::get_estimate,
    )
)]
pub struct ApiDoc;

struct BearerSecurity;

impl Modify for BearerSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        openapi
            .components
            .get_or_insert_with(Default::default)
            .add_security_scheme(
                "bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
    }
}
