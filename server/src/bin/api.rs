use std::sync::Arc;

use tower_http::services::{ServeDir, ServeFile};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Конфигурация — первой строкой: неверная настройка означает, что процесс
    // не поднялся, а не «упал на первом запросе через неделю».
    let settings = Arc::new(server::core::config::Settings::from_env()?);
    // `api healthcheck` — проверка для docker HEALTHCHECK: в образе нет curl,
    // зато есть этот же бинарь
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        return healthcheck(settings.port).await;
    }
    // guard держит файловый логгер до конца работы процесса
    let _log_guard = server::core::telemetry::init_tracing(&settings.log);
    server::core::telemetry::init_metrics(settings.metrics_addr)?;

    let pool = server::core::db::connect(settings.database_url.expose()).await?;
    server::core::db::MIGRATIONS.run(&pool).await?;

    // фоновые задачи (доставка писем, чистка токенов) — в этом же процессе;
    // после сигнала завершения воркер не берёт новую пачку писем
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    server::jobs::spawn(pool.clone(), &settings, shutdown_rx);

    let state = server::AppState {
        pool,
        settings: Arc::clone(&settings),
    };
    // SPA: API + статика фронта из одного бинаря — один дешёвый VPS, без nginx
    let app = server::app(state).fallback_service(
        ServeDir::new("web/dist").fallback(ServeFile::new("web/dist/index.html")),
    );

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", settings.port)).await?;
    tracing::info!("listening on {}", listener.local_addr()?);
    // with_connect_info — rate limiter'у нужен IP клиента
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(true);
    })
    .await?;
    Ok(())
}

/// Дёргает собственный /api/health/ready; ненулевой код = «контейнер нездоров»
async fn healthcheck(port: u16) -> anyhow::Result<()> {
    let url = format!("http://127.0.0.1:{port}/api/health/ready");
    let status = reqwest::get(&url).await?.status();
    anyhow::ensure!(status.is_success(), "{url} ответил {status}");
    Ok(())
}

/// Ctrl+C в терминале и SIGTERM от docker — оба означают «пора закругляться»
async fn shutdown_signal() {
    #[cfg(unix)]
    if let Ok(mut term) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
        return;
    }
    let _ = tokio::signal::ctrl_c().await;
}
