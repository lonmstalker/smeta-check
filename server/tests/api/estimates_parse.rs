//! Фоновый разбор смет: что получается из настоящих файлов и что происходит
//! с файлом, который прочитать нельзя.

use axum::http::StatusCode;
use server::estimates::parse;
use server::estimates::worker;

use crate::common::{fixture, spawn_app};

/// Все настоящие сметы из `tests/fixtures/estimates` — по одной проверке на файл
#[test]
fn every_fixture_is_readable_and_gives_lines() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/estimates");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("каталог фикстур")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("xlsx" | "xls")
            )
        })
        .collect();
    files.sort();
    assert!(files.len() >= 10, "смет для проверки слишком мало");

    let mut with_recognized = 0;
    for path in &files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let bytes = std::fs::read(path).expect("файл фикстуры");
        let lines =
            parse::parse(bytes).unwrap_or_else(|err| panic!("{name} не разобралась: {err:?}"));
        assert!(!lines.is_empty(), "{name}: ни одной строки");
        for line in &lines {
            assert!(
                !line.raw_text.is_empty(),
                "{name}: строка без сырого текста"
            );
            assert!(
                line.title.is_none() || line.is_recognized(),
                "{name}: у распознанной строки «{}» нет ни одного числа",
                line.raw_text
            );
        }
        if lines.iter().any(parse::ParsedLine::is_recognized) {
            with_recognized += 1;
        }
    }
    // часть файлов — не квартирные сметы со своей вёрсткой; требуем, чтобы
    // работы распознавались в подавляющем большинстве
    assert!(
        with_recognized * 10 >= files.len() * 8,
        "работы распознались только в {with_recognized} файлах из {}",
        files.len()
    );
}

#[tokio::test]
async fn uploaded_estimate_becomes_parsed_with_lines() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let content = fixture("sanuzel-novaya-moskva.xlsx");
    let created = app
        .post_file("/api/estimates", "Санузел.xlsx", &content, &token)
        .await;
    let id = created.body["id"].as_str().expect("id сметы").to_owned();

    let done = worker::run_pending(&app.pool, &app.files_dir)
        .await
        .expect("разбор");
    assert_eq!(done, 1, "воркер обязан взять смету в работу");

    let details = app.get_auth(&format!("/api/estimates/{id}"), &token).await;
    assert_eq!(details.status, StatusCode::OK);
    assert_eq!(details.body["status"], "parsed");
    let lines = details.body["lines"].as_array().expect("строки сметы");
    assert!(!lines.is_empty(), "у разобранной сметы обязаны быть строки");

    let recognized: Vec<_> = lines.iter().filter(|l| !l["title"].is_null()).collect();
    assert!(!recognized.is_empty(), "ни одна работа не распознана");
    let first = recognized[0];
    assert!(
        !first["quantity"].is_null() || !first["price"].is_null() || !first["total"].is_null(),
        "у распознанной строки нет чисел: {first}"
    );
    // нераспознанное не выбрасывается: сырой текст есть у каждой строки
    assert!(lines.iter().all(|l| l["raw_text"].is_string()));
}

#[tokio::test]
async fn broken_file_fails_with_a_readable_reason() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let created = app
        .post_file("/api/estimates", "smeta.xlsx", b"not an excel file", &token)
        .await;
    let id = created.body["id"].as_str().expect("id сметы").to_owned();

    worker::run_pending(&app.pool, &app.files_dir)
        .await
        .expect("разбор");

    let details = app.get_auth(&format!("/api/estimates/{id}"), &token).await;
    assert_eq!(details.body["status"], "failed");
    let reason = details.body["error"].as_str().unwrap_or_default();
    assert!(
        reason.contains("Excel"),
        "причина должна быть человеческой и на языке запроса: {reason}"
    );
    assert_eq!(details.body["lines"].as_array().map(Vec::len), Some(0));
}

#[tokio::test]
async fn estimate_stuck_in_parsing_is_picked_up_again() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let content = fixture("elektromontazh-nekrasov.xlsx");
    let created = app
        .post_file("/api/estimates", "Электрика.xlsx", &content, &token)
        .await;
    let id = created.body["id"].as_str().expect("id сметы").to_owned();

    // так выглядит смета, разбор которой оборвался вместе с процессом
    sqlx::query(
        "UPDATE estimates SET status = 'parsing', attempts = 1,
                              parsing_started_at = now() - make_interval(mins => 11)
         WHERE id = $1::uuid",
    )
    .bind(&id)
    .execute(&app.pool)
    .await
    .expect("подделать зависший разбор");

    worker::run_pending(&app.pool, &app.files_dir)
        .await
        .expect("разбор");

    let details = app.get_auth(&format!("/api/estimates/{id}"), &token).await;
    assert_eq!(
        details.body["status"], "parsed",
        "зависшая смета обязана дожеваться после рестарта"
    );
}

#[tokio::test]
async fn parsing_is_not_repeated_forever() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    app.post_file("/api/estimates", "smeta.xlsx", b"broken", &token)
        .await;

    // первый заход помечает смету failed, второму брать уже нечего
    assert_eq!(
        worker::run_pending(&app.pool, &app.files_dir).await.ok(),
        Some(1)
    );
    assert_eq!(
        worker::run_pending(&app.pool, &app.files_dir).await.ok(),
        Some(0)
    );
}
