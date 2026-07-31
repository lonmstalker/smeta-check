//! Подтверждение email: письмо при регистрации, одноразовая ссылка.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{spawn_app, token_from_letter as token_from};

#[tokio::test]
async fn registration_sends_email_and_link_verifies_once() {
    let app = spawn_app().await;
    let (token, email) = app.register_user().await;

    let me = app
        .request("GET", "/api/users/me", None, Some(&token), &[])
        .await;
    assert_eq!(me.body["email_verified"], json!(false));

    let letter = app.last_email_to(&email).await.expect("verification email");
    let verify_token = token_from(&letter);

    let res = app
        .post("/api/auth/verify-email", json!({ "token": verify_token }))
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let me = app
        .request("GET", "/api/users/me", None, Some(&token), &[])
        .await;
    assert_eq!(me.body["email_verified"], json!(true));

    // ссылка одноразовая
    let res = app
        .post("/api/auth/verify-email", json!({ "token": verify_token }))
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["error"]["code"], "error-invalid-token");
}

#[tokio::test]
async fn garbage_token_rejected() {
    let app = spawn_app().await;
    let res = app
        .post("/api/auth/verify-email", json!({ "token": "мусор" }))
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
}
