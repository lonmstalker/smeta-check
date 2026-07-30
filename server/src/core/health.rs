//! Проверки состояния — для оркестратора и для человека.
//!
//! live: процесс отвечает; ready: готов принимать трафик, то есть доступна БД;
//! version: что именно сейчас развёрнуто.
//!
//! SMTP и OAuth в readiness НЕ проверяем сознательно: авария внешнего почтового
//! сервиса не должна перезапускать приложение и снимать его с балансировщика.

use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::AppState;

/// Оркестратор ждёт быстрый ответ: дольше двух секунд БД можно считать недоступной
const READY_TIMEOUT: Duration = Duration::from_secs(2);

pub fn router() -> Router<AppState> {
    Router::new()
        // /api/health — исторический алиас live: им пользуются e2e и compose
        .route("/api/health", get(live))
        .route("/api/health/live", get(live))
        .route("/api/health/ready", get(ready))
        .route("/api/version", get(version))
}

#[utoipa::path(get, path = "/api/health/live", tag = "health",
    responses((status = 200, description = "процесс отвечает")))]
pub(crate) async fn live() -> &'static str {
    "ok"
}

#[utoipa::path(get, path = "/api/health/ready", tag = "health",
    responses((status = 200, description = "готов принимать трафик"),
              (status = 503, description = "база недоступна")))]
pub(crate) async fn ready(State(pool): State<PgPool>) -> StatusCode {
    match tokio::time::timeout(READY_TIMEOUT, sqlx::query("SELECT 1").execute(&pool)).await {
        Ok(Ok(_)) => StatusCode::OK,
        Ok(Err(err)) => {
            tracing::error!(error = ?err, "readiness: запрос к базе не прошёл");
            StatusCode::SERVICE_UNAVAILABLE
        }
        Err(_) => {
            tracing::error!("readiness: база не ответила за {READY_TIMEOUT:?}");
            StatusCode::SERVICE_UNAVAILABLE
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct VersionInfo {
    /// версия из Cargo.toml
    pub version: &'static str,
    /// коммит, из которого собран образ (build-arg GIT_SHA); dev-сборка — unknown
    pub commit: &'static str,
}

#[utoipa::path(get, path = "/api/version", tag = "health",
    responses((status = 200, body = VersionInfo)))]
pub(crate) async fn version() -> Json<VersionInfo> {
    Json(VersionInfo {
        version: env!("CARGO_PKG_VERSION"),
        commit: option_env!("GIT_SHA").unwrap_or("unknown"),
    })
}
