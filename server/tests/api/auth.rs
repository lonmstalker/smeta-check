//! Сессии: регистрация, вход, ротация refresh, выход.
//! Восстановление пароля — auth_recovery.rs, второй фактор — auth_totp.rs.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{PASSWORD, TestApp, spawn_app};

#[tokio::test]
async fn register_login_me_flow() {
    let app = spawn_app().await;
    let (token, email) = app.register_user().await;

    let me = app
        .request("GET", "/api/users/me", None, Some(&token), &[])
        .await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.body["email"], json!(email));
    assert_eq!(me.body["role"], json!("user"));

    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(login.body["requires_2fa"], json!(false));
    assert!(
        TestApp::refresh_token_of(&login).is_some(),
        "login must set refresh cookie"
    );
}

#[tokio::test]
async fn register_rejects_duplicates_and_weak_passwords() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;

    let dup = app
        .post(
            "/api/auth/register",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    assert_eq!(dup.status, StatusCode::CONFLICT);
    assert_eq!(dup.body["error"]["code"], "error-email-taken");

    let weak = app
        .post(
            "/api/auth/register",
            json!({ "email": "a@b.co", "password": "short" }),
        )
        .await;
    assert_eq!(weak.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(weak.body["error"]["code"], "error-password-short");

    let bad_email = app
        .post(
            "/api/auth/register",
            json!({ "email": "not-an-email", "password": PASSWORD }),
        )
        .await;
    assert_eq!(bad_email.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn concurrent_registration_of_same_email_is_a_conflict() {
    let app = spawn_app().await;
    let email = "race@test.local";
    let body = json!({ "email": email, "password": PASSWORD });
    // оба запроса проходят проверку «адрес свободен» до вставки; второго
    // ловит UNIQUE в БД — это конфликт, а не внутренняя ошибка
    let (first, second) = tokio::join!(
        app.post("/api/auth/register", body.clone()),
        app.post("/api/auth/register", body.clone()),
    );
    let mut statuses = [first.status, second.status];
    statuses.sort_by_key(|s| s.as_u16());
    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::CONFLICT]);
}

#[tokio::test]
async fn login_rejects_wrong_password() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let res = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": "wrong-pass-123" }),
        )
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.body["error"]["code"], "error-invalid-credentials");
}

#[tokio::test]
async fn refresh_rotates_and_old_token_dies() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    let first = TestApp::refresh_token_of(&login).unwrap();

    let cookie = format!("refresh_token={first}");
    let refreshed = app
        .request(
            "POST",
            "/api/auth/refresh",
            None,
            None,
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(refreshed.status, StatusCode::OK);
    let second = TestApp::refresh_token_of(&refreshed).unwrap();
    assert_ne!(first, second, "refresh must rotate the token");

    // старый токен погашен ротацией
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
}

#[tokio::test]
async fn logout_revokes_session() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    let token = TestApp::refresh_token_of(&login).unwrap();
    let cookie = format!("refresh_token={token}");

    let out = app
        .request(
            "POST",
            "/api/auth/logout",
            None,
            None,
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);

    let after = app
        .request(
            "POST",
            "/api/auth/refresh",
            None,
            None,
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED);
}
