//! Второй фактор (TOTP): включение, двухшаговый вход, выключение.

use axum::http::StatusCode;
use serde_json::json;
use totp_rs::{Algorithm, Secret, TOTP};

use crate::common::{PASSWORD, spawn_app};

/// Код «как в приложении-аутентификаторе» из выданного секрета
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "хелпер теста: паника валит тест — это и есть отчёт об ошибке"
)]
fn totp_code(secret_b32: &str) -> String {
    let secret = Secret::Encoded(secret_b32.to_owned()).to_bytes().unwrap();
    TOTP::new(Algorithm::SHA1, 6, 1, 30, secret, None, String::new())
        .unwrap()
        .generate_current()
        .unwrap()
}

#[tokio::test]
async fn totp_two_factor_full_flow() {
    let app = spawn_app().await;
    let (token, email) = app.register_user().await;

    let setup = app
        .post_auth("/api/auth/2fa/setup", json!({}), &token)
        .await;
    assert_eq!(setup.status, StatusCode::OK);
    let secret = setup.body["secret"].as_str().unwrap().to_owned();
    assert!(
        setup.body["otpauth_url"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/")
    );

    // включение с неверным кодом отклоняется
    let bad = app
        .post_auth(
            "/api/auth/2fa/enable",
            json!({ "secret": secret, "code": "000000" }),
            &token,
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);

    let ok = app
        .post_auth(
            "/api/auth/2fa/enable",
            json!({ "secret": secret, "code": totp_code(&secret) }),
            &token,
        )
        .await;
    assert_eq!(ok.status, StatusCode::NO_CONTENT, "{}", ok.body);

    // теперь вход двухшаговый
    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    assert_eq!(login.status, StatusCode::OK);
    assert_eq!(login.body["requires_2fa"], json!(true));
    assert!(
        login.body["access_token"].is_null(),
        "no tokens until 2fa passes"
    );
    let pending = login.body["pending_token"].as_str().unwrap();

    let verified = app
        .post(
            "/api/auth/2fa/verify",
            json!({ "pending_token": pending, "code": totp_code(&secret) }),
        )
        .await;
    assert_eq!(verified.status, StatusCode::OK);
    assert!(verified.body["access_token"].is_string());

    // выключение — по действующему коду
    let access = verified.body["access_token"].as_str().unwrap();
    let disabled = app
        .post_auth(
            "/api/auth/2fa/disable",
            json!({ "code": totp_code(&secret) }),
            access,
        )
        .await;
    assert_eq!(disabled.status, StatusCode::NO_CONTENT);

    let login = app
        .post(
            "/api/auth/login",
            json!({ "email": email, "password": PASSWORD }),
        )
        .await;
    assert_eq!(login.body["requires_2fa"], json!(false));
}
