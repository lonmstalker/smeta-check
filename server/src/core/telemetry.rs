//! Логи и метрики.
//!
//! Логи (tracing): dev — читаемый вывод; прод — LOG_FORMAT=json; LOG_DIR=<путь>
//! дублирует поток в файл с ротацией по дням. У каждого HTTP-запроса — request_id.
//! Метрики: Prometheus на отдельном порту (METRICS_ADDR, по умолчанию
//! 127.0.0.1:9464) — наружу не светится, снимается коллектором.

use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::http::HeaderName;
use axum::middleware::Next;
use axum::response::Response;
use tower_http::request_id::MakeRequestUuid;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::core::config::LogSettings;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Возвращает guard файлового логгера — держать живым до конца main
pub fn init_tracing(cfg: &LogSettings) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    // RUST_LOG остаётся за конфигом: это переменная самого tracing
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());

    let (file_layer, guard) = match &cfg.dir {
        Some(dir) => {
            let appender = tracing_appender::rolling::daily(dir, "server.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let layer = tracing_subscriber::fmt::layer().json().with_writer(writer);
            (Some(layer), Some(guard))
        }
        None => (None, None),
    };

    let registry = tracing_subscriber::registry().with(filter).with(file_layer);
    if cfg.json {
        registry
            .with(tracing_subscriber::fmt::layer().json())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
    guard
}

pub fn init_metrics(addr: std::net::SocketAddr) -> anyhow::Result<()> {
    metrics_exporter_prometheus::PrometheusBuilder::new()
        .with_http_listener(addr)
        .install()
        .map_err(|e| anyhow::anyhow!("не поднялся экспортер метрик на {addr}: {e}"))
}

pub fn request_id_layer() -> tower_http::request_id::SetRequestIdLayer<MakeRequestUuid> {
    tower_http::request_id::SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid)
}

/// Один лог-спан и метрики (счётчик + латентность) на каждый HTTP-запрос
pub async fn track_http(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    // matched path ("/api/items/{id}"), а не сырой URL — иначе кардинальность взорвётся
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let request_id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_owned();

    let started = Instant::now();
    let response = next.run(req).await;
    let elapsed = started.elapsed();
    let status = response.status().as_u16();

    tracing::info!(
        %method, path, status, request_id,
        elapsed_ms = elapsed.as_millis() as u64,
        "http"
    );
    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status.to_string()),
    ];
    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_request_duration_seconds", &labels).record(elapsed.as_secs_f64());
    response
}
