//! Список сессий и выход с других устройств.

use axum::http::StatusCode;
use serde_json::json;

use crate::common::{PASSWORD, TestApp, spawn_app};

/// Вход с указанным браузером: возвращает access-токен и refresh-cookie
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "хелпер теста: паника валит тест — это и есть отчёт об ошибке"
)]
async fn login_from(app: &TestApp, email: &str, user_agent: &str) -> (String, String) {
    let res = app
        .request(
            "POST",
            "/api/auth/login",
            Some(&json!({ "email": email, "password": PASSWORD })),
            None,
            &[("user-agent", user_agent)],
        )
        .await;
    assert_eq!(res.status, StatusCode::OK, "{}", res.body);
    let refresh = TestApp::refresh_token_of(&res).expect("в ответе refresh-cookie");
    (
        res.body["access_token"].as_str().unwrap().to_owned(),
        refresh,
    )
}

const CHROME_MAC: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) Chrome/120.0 Safari/537.36";
const FIREFOX_LINUX: &str =
    "Mozilla/5.0 (X11; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0";

#[tokio::test]
async fn sessions_list_shows_devices_and_marks_current() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let (token_a, refresh_a) = login_from(&app, &email, CHROME_MAC).await;
    login_from(&app, &email, FIREFOX_LINUX).await;

    let res = app
        .request(
            "GET",
            "/api/auth/sessions",
            None,
            Some(&token_a),
            &[("cookie", &format!("refresh_token={refresh_a}"))],
        )
        .await;
    assert_eq!(res.status, StatusCode::OK);
    let list = res.body.as_array().expect("список сессий");
    // регистрация тоже завела сессию, плюс два входа
    assert_eq!(list.len(), 3, "{}", res.body);

    let current: Vec<_> = list
        .iter()
        .filter(|s| s["current"] == json!(true))
        .collect();
    assert_eq!(current.len(), 1, "текущая сессия ровно одна: {}", res.body);
    assert_eq!(current[0]["client"], "Chrome, macOS");
    assert!(
        current[0]["created_at"].as_str().unwrap().contains('T'),
        "время в формате RFC 3339: {}",
        current[0]
    );
}

#[tokio::test]
async fn user_can_close_one_session_and_all_others() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let (token_a, refresh_a) = login_from(&app, &email, CHROME_MAC).await;
    let (token_b, _) = login_from(&app, &email, FIREFOX_LINUX).await;

    let cookie = format!("refresh_token={refresh_a}");
    let list = app
        .request(
            "GET",
            "/api/auth/sessions",
            None,
            Some(&token_a),
            &[("cookie", &cookie)],
        )
        .await;
    let other = list
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["current"] == json!(false))
        .expect("есть чужая сессия");
    let other_id = other["id"].as_str().unwrap();

    let res = app
        .delete_auth(&format!("/api/auth/sessions/{other_id}"), &token_a)
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);
    // повторный отзыв — как несуществующая сессия
    let res = app
        .delete_auth(&format!("/api/auth/sessions/{other_id}"), &token_a)
        .await;
    assert_eq!(res.status, StatusCode::NOT_FOUND);

    // «выйти на остальных устройствах» оставляет только текущую
    let res = app
        .request(
            "DELETE",
            "/api/auth/sessions",
            None,
            Some(&token_a),
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(res.status, StatusCode::NO_CONTENT);

    let list = app
        .request(
            "GET",
            "/api/auth/sessions",
            None,
            Some(&token_a),
            &[("cookie", &cookie)],
        )
        .await;
    assert_eq!(list.body.as_array().unwrap().len(), 1);
    assert_eq!(list.body[0]["current"], json!(true));
    // а токен закрытой сессии больше не обновляется
    let _ = token_b;
}

#[tokio::test]
async fn refresh_keeps_the_same_session() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let (token, refresh) = login_from(&app, &email, CHROME_MAC).await;

    let before = app
        .request(
            "GET",
            "/api/auth/sessions",
            None,
            Some(&token),
            &[("cookie", &format!("refresh_token={refresh}"))],
        )
        .await;
    let session_ids_before: Vec<_> = before
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].clone())
        .collect();

    let refreshed = app
        .request(
            "POST",
            "/api/auth/refresh",
            None,
            None,
            &[("cookie", &format!("refresh_token={refresh}"))],
        )
        .await;
    assert_eq!(refreshed.status, StatusCode::OK);
    let new_refresh = TestApp::refresh_token_of(&refreshed).expect("новая cookie");
    let new_token = refreshed.body["access_token"].as_str().unwrap();

    let after = app
        .request(
            "GET",
            "/api/auth/sessions",
            None,
            Some(new_token),
            &[("cookie", &format!("refresh_token={new_refresh}"))],
        )
        .await;
    let session_ids_after: Vec<_> = after
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].clone())
        .collect();
    assert_eq!(
        session_ids_before, session_ids_after,
        "ротация refresh не должна плодить новые сессии в списке"
    );
}
