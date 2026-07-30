//! Восстановление пароля: письмо из outbox, одноразовость токена,
//! отзыв старых сессий.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{PASSWORD, TestApp, spawn_app};

#[tokio::test]
async fn password_reset_full_flow() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    let old_refresh = TestApp::refresh_token_of(&login).unwrap();

    // не раскрываем существование адресов: и для чужого email — 202
    let res = app
        .post("/api/auth/forgot", json!({ "email": "ghost@test.local" }))
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED);

    let res = app
        .post("/api/auth/forgot", json!({ "email": email }))
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED);

    let body = app
        .last_email_to(&email)
        .await
        .expect("reset email in outbox");
    let token: String = body
        .split("token=")
        .nth(1)
        .expect("link with token")
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();

    let new_password = "brand-new-pass-1";
    let res = app
        .post(
            "/api/auth/reset",
            json!({ "token": token, "password": new_password }),
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    // старый пароль больше не работает, новый — работает
    let old = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    let new = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": new_password }),
        )
        .await;
    assert_eq!(new.status, StatusCode::OK);

    // все старые сессии отозваны
    let cookie = format!("refresh_token={old_refresh}");
    let replay = app
        .request(
            "POST",
            "/api/auth/refresh",
            None,
            None,
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(replay.status, StatusCode::UNAUTHORIZED);

    // токен сброса одноразовый
    let again = app
        .post(
            "/api/auth/reset",
            json!({ "token": token, "password": "another-pass-1" }),
        )
        .await;
    assert_eq!(again.status, StatusCode::UNPROCESSABLE_ENTITY);
}
