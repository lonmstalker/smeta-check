//! Ограничение частоты запросов по IP. Счётчики в памяти процесса (governor):
//! для одного инстанса этого достаточно; общий на кластер лимитер понадобится
//! не раньше, чем появится сам кластер.

use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use axum::Router;
use axum::extract::{ConnectInfo, Request};
use axum::middleware::Next;
use axum::response::IntoResponse;
use governor::{Quota, RateLimiter};

use crate::core::error::ApiError;

/// Как часто выбрасывать отлежавшиеся счётчики (раз в столько запросов)
const CLEANUP_EVERY: u32 = 1024;

type IpLimiter = RateLimiter<
    String,
    governor::state::keyed::DefaultKeyedStateStore<String>,
    governor::clock::DefaultClock,
>;

/// Навесить на маршруты роутера лимит «не больше `rpm` запросов в минуту
/// с одного IP». `rpm = 0` — без ограничений (dev, тесты).
/// `trust_proxy` — верить ли заголовку X-Forwarded-For (см. `client_ip`).
pub fn limit_per_ip<S: Clone + Send + Sync + 'static>(
    router: Router<S>,
    rpm: u32,
    trust_proxy: bool,
) -> Router<S> {
    let Some(rpm) = NonZeroU32::new(rpm) else {
        return router;
    };
    let limiter = Arc::new(IpLimiter::keyed(Quota::per_minute(rpm)));
    let requests = Arc::new(AtomicU32::new(0));
    router.route_layer(axum::middleware::from_fn(
        move |req: Request, next: Next| {
            let limiter = limiter.clone();
            let requests = requests.clone();
            async move {
                // счётчик заводится на каждый новый IP; без уборки таблица
                // растёт вместе с числом адресов (с IPv6 — неограниченно)
                if requests
                    .fetch_add(1, Ordering::Relaxed)
                    .is_multiple_of(CLEANUP_EVERY)
                {
                    limiter.retain_recent();
                }
                if limiter.check_key(&client_ip(&req, trust_proxy)).is_err() {
                    metrics::counter!("rate_limited_total").increment(1);
                    return ApiError::too_many_requests().into_response();
                }
                next.run(req).await
            }
        },
    ))
}

/// С прокси (TRUST_PROXY=true) последний адрес в X-Forwarded-For дописан
/// нашим же Caddy — ему верим. Без прокси заголовок подделывает кто угодно,
/// поэтому берём только адрес соединения.
fn client_ip(req: &Request, trust_proxy: bool) -> String {
    let forwarded = trust_proxy
        .then(|| {
            req.headers()
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.rsplit(',').next())
                .map(|ip| ip.trim().to_owned())
        })
        .flatten();
    forwarded
        .or_else(|| {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}
