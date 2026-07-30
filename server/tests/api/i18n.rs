//! Проверки локализации: ответы строго на языке клиента, словари полные.

use std::collections::BTreeSet;

use axum::http::StatusCode;
use serde_json::json;

use crate::common::spawn_app;

#[tokio::test]
async fn api_errors_follow_accept_language_with_plurals() {
    let app = spawn_app().await;
    let weak = json!({ "email": "a@b.co", "password": "short" });

    let ru = app
        .request(
            "POST",
            "/api/auth/register",
            Some(&weak),
            None,
            &[("accept-language", "ru")],
        )
        .await;
    assert_eq!(ru.status, StatusCode::UNPROCESSABLE_ENTITY);
    let ru_message = ru.body["error"]["message"].as_str().unwrap();
    // «8 символов» — правильная русская множественная форма
    assert!(
        ru_message.contains("символов"),
        "ru message localized: {ru_message}"
    );

    let en = app
        .request(
            "POST",
            "/api/auth/register",
            Some(&weak),
            None,
            &[("accept-language", "en-US,en")],
        )
        .await;
    let en_message = en.body["error"]["message"].as_str().unwrap();
    assert!(
        en_message.contains("characters"),
        "en message localized: {en_message}"
    );

    // без заголовка отвечаем на языке по умолчанию (русский)
    let default = app.post("/api/auth/register", weak).await;
    let message = default.body["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("символов"),
        "default lang is ru: {message}"
    );
}

#[test]
fn locales_have_identical_key_sets() {
    use server::core::i18n::{ALL_LANGS, message_keys};
    let reference: BTreeSet<_> = message_keys(ALL_LANGS[0].0).into_iter().collect();
    assert!(!reference.is_empty());
    for &(lang, code) in &ALL_LANGS[1..] {
        let keys: BTreeSet<_> = message_keys(lang).into_iter().collect();
        let missing: Vec<_> = reference.difference(&keys).collect();
        let extra: Vec<_> = keys.difference(&reference).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "locale '{code}' differs: missing {missing:?}, extra {extra:?}"
        );
    }
}
