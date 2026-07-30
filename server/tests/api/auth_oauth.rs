//! Вход через внешнего провайдера. Настоящий провайдер в тестах не нужен:
//! обмен кода на профиль — это HTTP к чужому серверу, а вся наша логика
//! живёт в find_or_create_user, её и проверяем. Сверху — два HTTP-теста на
//! то, что чужой callback без state-куки не пускают.

use axum::http::StatusCode;
use server::core::config::{OauthProvider, Secret};

use crate::common::{spawn_app, spawn_app_with};

fn provider(trust_email: bool) -> OauthProvider {
    OauthProvider {
        name: "testprovider".into(),
        client_id: "id".into(),
        client_secret: Secret::from("secret".to_owned()),
        auth_url: "https://provider.test/authorize".into(),
        token_url: "https://provider.test/token".into(),
        userinfo_url: "https://provider.test/me".into(),
        scope: "email".into(),
        id_pointer: "/id".into(),
        email_pointer: Some("/email".into()),
        trust_email,
    }
}

#[tokio::test]
async fn second_login_finds_the_same_user() {
    let app = spawn_app().await;
    let cfg = provider(true);

    let first =
        server::auth::oauth::find_or_create_user(&app.pool, &cfg, "42", Some("Vasya@Example.com"))
            .await
            .unwrap();
    // адрес от провайдера нормализуется в нижний регистр и считается подтверждённым
    assert_eq!(first.email, "vasya@example.com");
    assert!(first.email_verified);

    let second =
        server::auth::oauth::find_or_create_user(&app.pool, &cfg, "42", Some("vasya@example.com"))
            .await
            .unwrap();
    assert_eq!(first.id, second.id, "та же привязка — тот же пользователь");
}

#[tokio::test]
async fn untrusted_email_does_not_hijack_existing_account() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;

    // провайдер без TRUST_EMAIL: чужой адрес не даёт доступа к чужому аккаунту
    let stranger =
        server::auth::oauth::find_or_create_user(&app.pool, &provider(false), "7", Some(&email))
            .await
            .unwrap();
    assert_ne!(stranger.email, email);
    assert_eq!(stranger.email, "testprovider.7@oauth.local");

    // а с TRUST_EMAIL адрес проверен провайдером — привязка к тому же аккаунту
    let owner =
        server::auth::oauth::find_or_create_user(&app.pool, &provider(true), "8", Some(&email))
            .await
            .unwrap();
    assert_eq!(owner.email, email);
}

#[tokio::test]
async fn user_without_email_gets_synthetic_address() {
    let app = spawn_app().await;
    let user = server::auth::oauth::find_or_create_user(&app.pool, &provider(true), "99", None)
        .await
        .unwrap();
    assert_eq!(user.email, "testprovider.99@oauth.local");
}

#[tokio::test]
async fn callback_without_state_cookie_is_rejected() {
    // провайдер настроен, но state из редиректа не совпадает с cookie
    let app = spawn_app_with(|settings| {
        settings.oauth.insert("fake".into(), provider(true));
    })
    .await;
    let res = app
        .get("/api/auth/oauth/fake/callback?code=abc&state=guessed")
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(res.body["error"]["code"], "error-oauth-failed");
}

#[tokio::test]
async fn unknown_provider_is_not_configured() {
    let app = spawn_app().await;
    let res = app.get("/api/auth/oauth/nosuch/start").await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["error"]["code"], "error-oauth-not-configured");
}
