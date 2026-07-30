//! Записи и права на них: видит и меняет их только владелец.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{PASSWORD, spawn_app};

#[tokio::test]
async fn create_requires_login_and_appears_in_list() {
    let app = spawn_app().await;

    let res = app
        .post("/api/items", json!({ "title": "Без входа" }))
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(app.get("/api/items").await.status, StatusCode::UNAUTHORIZED);

    let (token, _) = app.register_user().await;
    let res = app
        .post_auth("/api/items", json!({ "title": "Первая" }), &token)
        .await;
    assert_eq!(res.status, StatusCode::CREATED);

    let res = app.get_auth("/api/items", &token).await;
    assert_eq!(res.status, StatusCode::OK);
    let titles: Vec<_> = res.body["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| &i["title"])
        .collect();
    assert!(
        titles.contains(&&json!("Первая")),
        "created item must be listed: {}",
        res.body
    );
}

#[tokio::test]
async fn list_paginates_by_cursor() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    for n in 1..=3 {
        app.post_auth(
            "/api/items",
            json!({ "title": format!("Запись {n}") }),
            &token,
        )
        .await;
    }

    let page = app.get_auth("/api/items?limit=2", &token).await;
    assert_eq!(page.body["items"].as_array().unwrap().len(), 2);
    let cursor = page.body["next_cursor"].as_i64().expect("has next page");

    let rest = app
        .get_auth(&format!("/api/items?limit=2&cursor={cursor}"), &token)
        .await;
    assert_eq!(rest.body["items"].as_array().unwrap().len(), 1);
    assert!(
        rest.body["next_cursor"].is_null(),
        "last page has no cursor"
    );
}

#[tokio::test]
async fn empty_title_rejected() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let res = app
        .post_auth("/api/items", json!({ "title": "   " }), &token)
        .await;
    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["error"]["code"], "error-title-empty");
    assert_eq!(
        res.body["error"]["fields"][0]["field"], "title",
        "ошибка обязана указывать на поле: {}",
        res.body
    );
}

#[tokio::test]
async fn user_sees_and_deletes_only_own_items() {
    let app = spawn_app().await;
    let (alice, _) = app.register_user().await;
    let (bob, _) = app.register_user().await;

    let created = app
        .post_auth("/api/items", json!({ "title": "Запись Алисы" }), &alice)
        .await;
    let id = created.body["id"].as_i64().unwrap();

    // Боб не видит чужую запись в своём списке
    let bobs_list = app.get_auth("/api/items", &bob).await;
    assert!(
        bobs_list.body["items"].as_array().unwrap().is_empty(),
        "чужие записи не должны попадать в список: {}",
        bobs_list.body
    );

    // и не может её удалить: ответ такой же, как для несуществующей записи
    let foreign = app.delete_auth(&format!("/api/items/{id}"), &bob).await;
    let missing = app.delete_auth("/api/items/999999", &bob).await;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.status, foreign.status);
    assert_eq!(missing.body, foreign.body, "ответы обязаны быть неотличимы");

    // запись на месте, владелец удаляет её сам
    let alices_list = app.get_auth("/api/items", &alice).await;
    assert_eq!(alices_list.body["items"].as_array().unwrap().len(), 1);
    let own = app.delete_auth(&format!("/api/items/{id}"), &alice).await;
    assert_eq!(own.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn admin_can_delete_foreign_item() {
    let app = spawn_app().await;
    let (owner, _) = app.register_user().await;
    let (_, admin_email) = app.register_user().await;

    let created = app
        .post_auth("/api/items", json!({ "title": "Под модерацию" }), &owner)
        .await;
    let id = created.body["id"].as_i64().unwrap();

    app.promote_to_admin(&admin_email).await;
    // роль сидит в JWT — после смены роли нужен новый токен
    let relogin = app
        .post(
            "/api/auth/login",
            json!({ "email": admin_email, "password": PASSWORD }),
        )
        .await;
    let admin_token = relogin.body["access_token"].as_str().unwrap();

    let res = app
        .delete_auth(&format!("/api/items/{id}"), admin_token)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    let res = app
        .delete_auth(&format!("/api/items/{id}"), admin_token)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);
}
