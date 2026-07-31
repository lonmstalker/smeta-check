//! Разбор сметы с фотографии: нейросеть подменена фейковым провайдером,
//! настоящий ключ не нужен. Проверяем не «умеет ли модель читать» (это
//! замер F0), а нашу часть: строки, попытки и честные отказы.

use axum::http::StatusCode;
use server::estimates::worker;

use crate::common::{TestApp, spawn_app_with};
use crate::llm_stub::LlmStub;

/// Минимальный jpeg: до настоящего распознавания дело не доходит
const JPEG: &[u8] = b"\xff\xd8\xff\xe0\x00\x10JFIF photo of an estimate";

/// Приложение с включённой нейросетью, смотрящей в фейкового провайдера
async fn app_with(stub: &LlmStub) -> TestApp {
    let base_url = stub.base_url.clone();
    spawn_app_with(move |settings| {
        settings.llm.base_url = base_url;
        settings.llm.api_key = Some("test-llm-key".to_owned().into());
    })
    .await
}

/// Загрузить фото и вернуть id сметы
#[allow(
    clippy::expect_used,
    reason = "хелпер теста: паника валит тест — это и есть отчёт об ошибке"
)]
async fn upload_photo(app: &TestApp) -> String {
    let (token, _) = app.register_verified_user().await;
    let res = app
        .post_file("/api/estimates", "Смета.jpg", JPEG, &token)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "загрузка: {}", res.body);
    res.body["id"].as_str().expect("id сметы").to_owned()
}

#[allow(
    clippy::expect_used,
    reason = "хелпер теста: паника валит тест — это и есть отчёт об ошибке"
)]
async fn status_of(app: &TestApp, id: &str) -> (String, i32, Option<String>) {
    sqlx::query_as("SELECT status, attempts, error_key FROM estimates WHERE id = $1::uuid")
        .bind(id)
        .fetch_one(&app.pool)
        .await
        .expect("состояние сметы")
}

#[tokio::test]
async fn photo_becomes_estimate_lines() {
    let stub = LlmStub::answering(
        r#"```json
        {"lines":[
          {"name":"Штукатурка стен","unit":"м2","quantity":10,"price":350,"total":3500},
          {"name":"СТЕНЫ"},
          {"name":"Стяжка пола","unit":"м2","quantity":10,"price":350,"total":9999}
        ],"unreadable":["нижняя строка смазана"]}
        ```"#,
    )
    .await;
    let app = app_with(&stub).await;
    let id = upload_photo(&app).await;

    let done = worker::run_pending(&app.pool, &app.files_dir, &app.settings)
        .await
        .expect("разбор");

    assert_eq!(done, 1);
    assert_eq!(stub.calls(), 1, "на одну смету — один вызов нейросети");
    let (status, _, _) = status_of(&app, &id).await;
    assert_eq!(status, "parsed");

    let lines: Vec<(String, Option<String>, Option<f64>)> = sqlx::query_as(
        "SELECT raw_text, title, price FROM estimate_lines
         WHERE estimate_id = $1::uuid ORDER BY position",
    )
    .bind(&id)
    .fetch_all(&app.pool)
    .await
    .expect("строки сметы");
    assert_eq!(
        lines.len(),
        4,
        "ни одна строка не должна потеряться: {lines:?}"
    );
    assert_eq!(lines[0].1.as_deref(), Some("Штукатурка стен"));
    assert_eq!(lines[0].2, Some(350.0));
    // заголовок раздела остаётся строкой без чисел
    assert_eq!(lines[1].1.as_deref(), Some("СТЕНЫ"));
    // 10 × 350 ≠ 9999 — числа спорят, строка уходит в нераспознанные
    assert_eq!(lines[2].1, None, "неверное число хуже отсутствующего");
    assert!(lines[2].0.contains("Стяжка пола"), "текст строки сохранён");
    assert_eq!(lines[3].0, "нижняя строка смазана");
}

#[tokio::test]
async fn unreadable_photo_asks_to_take_another_shot_after_three_tries() {
    let stub = LlmStub::answering("извините, на фотографии не видно сметы").await;
    let app = app_with(&stub).await;
    let id = upload_photo(&app).await;

    for attempt in 1..=3 {
        worker::run_pending(&app.pool, &app.files_dir, &app.settings)
            .await
            .expect("разбор");
        let (status, attempts, _) = status_of(&app, &id).await;
        assert_eq!(attempts, attempt, "попытка обязана тратиться");
        if attempt < 3 {
            assert_eq!(status, "uploaded", "до третьей попытки смета ждёт");
        }
    }

    let (status, _, error_key) = status_of(&app, &id).await;
    assert_eq!(status, "failed");
    assert_eq!(
        error_key.as_deref(),
        Some("error-estimate-photo-unreadable")
    );
    assert_eq!(stub.calls(), 3, "три попытки — три вызова");
}

#[tokio::test]
async fn provider_failure_does_not_burn_the_attempt() {
    let stub = LlmStub::failing(StatusCode::UNAUTHORIZED).await;
    let app = app_with(&stub).await;
    let id = upload_photo(&app).await;

    worker::run_pending(&app.pool, &app.files_dir, &app.settings)
        .await
        .expect("разбор");

    let (status, attempts, error_key) = status_of(&app, &id).await;
    assert_eq!(attempts, 0, "неверный ключ провайдера — не вина сметы");
    assert_eq!(status, "parsing", "смета жива и дождётся следующего захода");
    assert_eq!(error_key, None, "пользователю ошибку не показываем");
}

#[tokio::test]
async fn exhausted_token_budget_leaves_the_estimate_waiting() {
    let stub = LlmStub::answering("этот ответ никто не должен получить").await;
    let app = app_with(&stub).await;
    let id = upload_photo(&app).await;
    sqlx::query(
        "INSERT INTO llm_calls (model, prompt_tokens, completion_tokens) VALUES ($1, $2, 0)",
    )
    .bind("test/model")
    .bind(2_000_000_i32)
    .execute(&app.pool)
    .await
    .expect("потолок выбран");

    worker::run_pending(&app.pool, &app.files_dir, &app.settings)
        .await
        .expect("разбор");

    let (status, attempts, _) = status_of(&app, &id).await;
    assert_eq!(stub.calls(), 0, "при выбранном потолке в сеть не ходим");
    assert_eq!(attempts, 0, "бюджет кончился — попытка не сгорает");
    assert_eq!(status, "parsing");
}

#[tokio::test]
async fn an_estimate_stuck_for_a_day_gets_a_readable_ending() {
    let stub = LlmStub::failing(StatusCode::SERVICE_UNAVAILABLE).await;
    let app = app_with(&stub).await;
    let id = upload_photo(&app).await;
    sqlx::query(
        "UPDATE estimates SET created_at = now() - interval '25 hours' WHERE id = $1::uuid",
    )
    .bind(&id)
    .execute(&app.pool)
    .await
    .expect("состарили смету");

    worker::run_pending(&app.pool, &app.files_dir, &app.settings)
        .await
        .expect("разбор");

    let (status, _, error_key) = status_of(&app, &id).await;
    assert_eq!(status, "failed", "вечного «разбирается» не бывает");
    assert_eq!(error_key.as_deref(), Some("error-estimate-later"));
}

#[tokio::test]
async fn only_one_photo_per_tick() {
    let stub =
        LlmStub::answering(r#"{"lines":[{"name":"Работа","quantity":1,"price":100}]}"#).await;
    let app = app_with(&stub).await;
    let (token, _) = app.register_verified_user().await;
    for n in 0..2 {
        let res = app
            .post_file("/api/estimates", &format!("Смета-{n}.jpg"), JPEG, &token)
            .await;
        assert_eq!(
            res.status,
            StatusCode::CREATED,
            "загрузка {n}: {}",
            res.body
        );
    }

    let done = worker::run_pending(&app.pool, &app.files_dir, &app.settings)
        .await
        .expect("разбор");

    assert_eq!(done, 1, "тяжёлое фото не должно тянуть за собой пачку");
    assert_eq!(stub.calls(), 1);
}
