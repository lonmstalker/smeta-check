//! Клиент нейросети: учёт, дневной потолок и честное разделение ошибок.
//! Настоящий ключ здесь не нужен и не используется — только фейковый
//! провайдер из `llm_stub.rs`.

use axum::http::StatusCode;

use server::core::llm::{self, Image, LlmError};

use crate::common::{TestApp, spawn_app_with};
use crate::llm_stub::{LlmStub, STUB_COMPLETION_TOKENS, STUB_PROMPT_TOKENS};

/// Приложение, у которого нейросеть включена и смотрит в фейкового провайдера
async fn app_with_llm(stub: &LlmStub) -> TestApp {
    let base_url = stub.base_url.clone();
    spawn_app_with(move |settings| {
        settings.llm.base_url = base_url;
        settings.llm.api_key = Some("test-llm-key".to_owned().into());
        settings.llm.model = "test/model".into();
    })
    .await
}

fn photo() -> Image<'static> {
    Image {
        bytes: b"\xff\xd8\xff\xe0 fake jpeg",
        mime: "image/jpeg",
    }
}

#[tokio::test]
async fn answer_comes_back_and_tokens_are_recorded() {
    let stub = LlmStub::answering("готовый ответ модели").await;
    let app = app_with_llm(&stub).await;

    let answer = llm::complete(&app.pool, &app.settings, "перепиши смету", &[photo()])
        .await
        .expect("модель ответила");

    assert_eq!(answer, "готовый ответ модели");
    assert_eq!(stub.calls(), 1, "вызов должен уйти провайдеру ровно один");
    let (model, prompt, completion): (String, i32, i32) =
        sqlx::query_as("SELECT model, prompt_tokens, completion_tokens FROM llm_calls")
            .fetch_one(&app.pool)
            .await
            .expect("вызов записан в учёт");
    assert_eq!(model, "test/model");
    assert_eq!(
        (prompt, completion),
        (STUB_PROMPT_TOKENS, STUB_COMPLETION_TOKENS)
    );
}

#[tokio::test]
async fn exhausted_daily_cap_stops_the_call_before_the_network() {
    let stub = LlmStub::answering("этот ответ никто не должен получить").await;
    let app = app_with_llm(&stub).await;
    // потолок выбран сегодняшними вызовами
    sqlx::query(
        "INSERT INTO llm_calls (model, prompt_tokens, completion_tokens) VALUES ($1, $2, 0)",
    )
    .bind("test/model")
    .bind(2_000_000_i32)
    .execute(&app.pool)
    .await
    .expect("учёт заполнен");

    let error = llm::complete(&app.pool, &app.settings, "перепиши смету", &[photo()])
        .await
        .expect_err("потолок обязан остановить вызов");

    assert!(matches!(error, LlmError::Capped), "не тот отказ: {error:?}");
    assert_eq!(stub.calls(), 0, "при выбранном потолке в сеть не ходим");
}

#[tokio::test]
async fn yesterdays_tokens_do_not_count_against_today() {
    let stub = LlmStub::answering("сегодня снова можно").await;
    let app = app_with_llm(&stub).await;
    sqlx::query(
        "INSERT INTO llm_calls (model, prompt_tokens, completion_tokens, created_at)
         VALUES ($1, $2, 0, now() - interval '1 day')",
    )
    .bind("test/model")
    .bind(2_000_000_i32)
    .execute(&app.pool)
    .await
    .expect("учёт заполнен");

    let answer = llm::complete(&app.pool, &app.settings, "перепиши смету", &[photo()]).await;

    assert!(
        answer.is_ok(),
        "вчерашние токены не должны запирать сегодня"
    );
}

#[tokio::test]
async fn provider_error_is_not_the_users_fault() {
    let stub = LlmStub::failing(StatusCode::UNAUTHORIZED).await;
    let app = app_with_llm(&stub).await;

    let error = llm::complete(&app.pool, &app.settings, "перепиши смету", &[photo()])
        .await
        .expect_err("401 от провайдера — это отказ");

    assert!(
        matches!(error, LlmError::Http(401)),
        "не тот отказ: {error:?}"
    );
    let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM llm_calls")
        .fetch_one(&app.pool)
        .await
        .expect("учёт читается");
    assert_eq!(recorded, 0, "несостоявшийся вызов в учёт не пишется");
}

#[tokio::test]
async fn without_key_llm_is_disabled() {
    let app = spawn_app_with(|_| {}).await;

    assert!(
        !llm::enabled(&app.settings),
        "без ключа нейросеть выключена"
    );
    let error = llm::complete(&app.pool, &app.settings, "перепиши смету", &[photo()])
        .await
        .expect_err("без ключа звонить некуда");
    assert!(
        matches!(error, LlmError::Disabled),
        "не тот отказ: {error:?}"
    );
}
