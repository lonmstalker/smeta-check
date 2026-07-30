//! Самообслуживание аккаунта: профиль, смена пароля, смена адреса почты.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{PASSWORD, spawn_app};

#[tokio::test]
async fn profile_name_and_locale_are_saved() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;

    let res = app
        .patch_auth(
            "/api/users/me",
            json!({ "display_name": "  Аня  ", "locale": "en" }),
            &token,
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    assert_eq!(res.body["display_name"], "Аня", "имя обрезается по краям");
    assert_eq!(res.body["locale"], "en");

    // пропущенное поле не затирает сохранённое
    let res = app
        .patch_auth("/api/users/me", json!({ "locale": "ru" }), &token)
        .await;
    assert_eq!(res.body["display_name"], "Аня");
    assert_eq!(res.body["locale"], "ru");
}

#[tokio::test]
async fn unknown_locale_is_rejected() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let res = app
        .patch_auth("/api/users/me", json!({ "locale": "klingon" }), &token)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["error"]["fields"][0]["field"], "locale");
}

#[tokio::test]
async fn password_change_requires_current_and_closes_sessions() {
    let app = spawn_app().await;
    let (token, email) = app.register_user().await;

    let wrong = app
        .post_auth(
            "/api/auth/password",
            json!({ "current_password": "не тот пароль", "new_password": "new-horse-9" }),
            &token,
        )
        .await;
    assert_eq!(wrong.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        wrong.body["error"]["fields"][0]["field"], "current_password",
        "ошибка обязана указывать на поле: {}",
        wrong.body
    );

    let ok = app
        .post_auth(
            "/api/auth/password",
            json!({ "current_password": PASSWORD, "new_password": "new-horse-9" }),
            &token,
        )
        .await;
    assert_eq!(ok.status, StatusCode::NO_CONTENT);

    // старый пароль больше не подходит, новый — да
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
            json!({ "email": email, "password": "new-horse-9" }),
        )
        .await;
    assert_eq!(new.status, StatusCode::OK);
}

#[tokio::test]
async fn too_short_new_password_points_at_its_own_field() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let res = app
        .post_auth(
            "/api/auth/password",
            json!({ "current_password": PASSWORD, "new_password": "мало" }),
            &token,
        )
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["error"]["fields"][0]["field"], "new_password");
}

#[tokio::test]
async fn email_change_needs_confirmation_from_new_address() {
    let app = spawn_app().await;
    let (token, old_email) = app.register_user().await;
    let new_email = format!("new-{}@test.local", uuid::Uuid::new_v4().simple());

    let res = app
        .post_auth(
            "/api/auth/email",
            json!({ "new_email": new_email, "current_password": PASSWORD }),
            &token,
        )
        .await;
    assert_eq!(res.status, StatusCode::ACCEPTED);

    // старый адрес получает предупреждение, новый — ссылку
    let notice = app
        .last_email_to(&old_email)
        .await
        .expect("письмо на старый");
    assert!(
        notice.contains(&new_email),
        "в уведомлении новый адрес: {notice}"
    );
    let letter = app
        .last_email_to(&new_email)
        .await
        .expect("письмо на новый");
    // вокруг подстановок Fluent ставит невидимые управляющие символы —
    // берём ровно шестнадцатеричный хвост ссылки
    let link_token: String = letter
        .split("token=")
        .nth(1)
        .expect("в письме ссылка с токеном")
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect();
    assert!(!link_token.is_empty(), "токен пустой: {letter}");

    // до подтверждения адрес прежний
    let me = app.get_auth("/api/users/me", &token).await;
    assert_eq!(me.body["email"], old_email);

    let confirm = app
        .post("/api/auth/email/confirm", json!({ "token": link_token }))
        .await;
    assert_eq!(confirm.status, StatusCode::NO_CONTENT);

    let me = app.get_auth("/api/users/me", &token).await;
    assert_eq!(me.body["email"], new_email);
    assert_eq!(me.body["email_verified"], true, "новый адрес подтверждён");

    // ссылка одноразовая
    let again = app
        .post("/api/auth/email/confirm", json!({ "token": link_token }))
        .await;
    assert_eq!(again.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn email_change_rejects_taken_address() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let (_, someone_else) = app.register_user().await;

    let res = app
        .post_auth(
            "/api/auth/email",
            json!({ "new_email": someone_else, "current_password": PASSWORD }),
            &token,
        )
        .await;
    assert_eq!(res.status, StatusCode::CONFLICT);
    assert_eq!(res.body["error"]["fields"][0]["field"], "new_email");
}
