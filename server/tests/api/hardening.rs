//! Обвязка, которая защищает приложение снаружи: проверки состояния,
//! ограничение частоты запросов, доверие к прокси, чужой Origin, размер тела.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{spawn_app, spawn_app_with};

#[tokio::test]
async fn health_endpoints_report_state() {
    let app = spawn_app().await;
    assert_eq!(app.get("/api/health/live").await.status, StatusCode::OK);
    // ready ходит в базу: если она недоступна, ответ был бы 503
    assert_eq!(app.get("/api/health/ready").await.status, StatusCode::OK);

    let version = app.get("/api/version").await;
    assert_eq!(version.status, StatusCode::OK);
    assert!(
        version.body["version"].is_string() && version.body["commit"].is_string(),
        "версия и коммит обязаны быть в ответе: {}",
        version.body
    );
}

#[tokio::test]
async fn forwarded_ip_is_ignored_without_proxy() {
    // без TRUST_PROXY заголовок X-Forwarded-For подделывает кто угодно, поэтому
    // лимит должен считаться по адресу соединения, а не по заголовку
    let app = spawn_app_with(|s| {
        s.trust_proxy = false;
        s.rate_limit_auth_rpm = 2;
    })
    .await;
    let body = json!({ "email": "nobody@example.com" });
    for n in 0..2 {
        let res = app
            .request(
                "POST",
                "/api/auth/forgot",
                Some(&body),
                None,
                &[("x-forwarded-for", &format!("10.0.0.{n}"))],
            )
            .await;
        assert_eq!(res.status, StatusCode::ACCEPTED);
    }
    let res = app
        .request(
            "POST",
            "/api/auth/forgot",
            Some(&body),
            None,
            &[("x-forwarded-for", "10.0.0.99")],
        )
        .await;
    assert_eq!(
        res.status,
        StatusCode::TOO_MANY_REQUESTS,
        "подделанный X-Forwarded-For не должен обходить лимит"
    );
}

#[tokio::test]
async fn foreign_origin_cannot_touch_session_routes() {
    let app = spawn_app_with(|s| s.public_url = "https://app.example.com".into()).await;
    let body = json!({ "email": "nobody@example.com" });

    let evil = app
        .request(
            "POST",
            "/api/auth/forgot",
            Some(&body),
            None,
            &[("origin", "https://evil.example")],
        )
        .await;
    assert_eq!(evil.status, StatusCode::FORBIDDEN);

    let ours = app
        .request(
            "POST",
            "/api/auth/forgot",
            Some(&body),
            None,
            &[("origin", "https://app.example.com")],
        )
        .await;
    assert_eq!(ours.status, StatusCode::ACCEPTED);
}

#[tokio::test]
async fn oversized_body_is_rejected() {
    let app = spawn_app().await;
    // auth-ручки ограничены 16 КиБ: пароль такой длины — это не пароль
    let huge = json!({ "email": "a@b.co", "password": "x".repeat(64 * 1024) });
    let res = app.post("/api/auth/login", huge).await;
    assert_eq!(res.status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn auth_requests_are_rate_limited_per_ip() {
    let app = spawn_app().await;
    let body = json!({ "email": "nobody@example.com" });
    let ip = [("x-forwarded-for", "10.9.9.9")];
    // лимит по умолчанию 30 в минуту: первые 30 проходят, 31-й режется
    for _ in 0..30 {
        let res = app
            .request("POST", "/api/auth/forgot", Some(&body), None, &ip)
            .await;
        assert_eq!(res.status, StatusCode::ACCEPTED);
    }
    let res = app
        .request("POST", "/api/auth/forgot", Some(&body), None, &ip)
        .await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.body["error"]["code"], "error-too-many-requests");

    // другой IP лимитом не задет
    let other = app
        .request(
            "POST",
            "/api/auth/forgot",
            Some(&body),
            None,
            &[("x-forwarded-for", "10.9.9.10")],
        )
        .await;
    assert_eq!(other.status, StatusCode::ACCEPTED);
}

#[tokio::test]
async fn cors_headers_appear_when_origins_configured() {
    let app = spawn_app_with(|s| s.cors_origins = vec!["https://app.example.com".into()]).await;
    // preflight PATCH: внешний клиент правит профиль (/api/users/me)
    let res = app
        .request(
            "OPTIONS",
            "/api/users/me",
            None,
            None,
            &[
                ("origin", "https://app.example.com"),
                ("access-control-request-method", "PATCH"),
            ],
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    let allowed = res.headers["access-control-allow-methods"]
        .to_str()
        .unwrap();
    for method in ["GET", "POST", "PATCH", "DELETE"] {
        assert!(allowed.contains(method), "{method} нет в CORS: {allowed}");
    }
}
