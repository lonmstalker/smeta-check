//! Загрузка смет: кто может грузить, что принимаем и что видит владелец.

use axum::http::StatusCode;

use crate::common::{fixture, spawn_app, spawn_app_with};

/// Маленькая настоящая смета — на ней проверяем путь целиком
const SMALL_XLSX: &str = "obshchaya-smeta-designershelp.xlsx";

#[tokio::test]
async fn uploaded_estimate_is_listed_and_saved_to_disk() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    let content = fixture(SMALL_XLSX);

    let res = app
        .post_file("/api/estimates", "Смета бригады.xlsx", &content, &token)
        .await;
    assert_eq!(res.status, StatusCode::CREATED, "ответ: {}", res.body);
    assert_eq!(res.body["status"], "uploaded");
    assert_eq!(res.body["file_name"], "Смета бригады.xlsx");
    assert_eq!(res.body["size_bytes"].as_i64(), Some(content.len() as i64));
    let id = res.body["id"].as_str().expect("id сметы").to_owned();

    // файл действительно лежит на диске под именем от id
    let saved = std::fs::read(app.files_dir.join(format!("{id}.xlsx"))).expect("файл сметы");
    assert_eq!(saved, content, "на диск попал не тот файл");

    let list = app.get_auth("/api/estimates", &token).await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.body[0]["id"], id, "смета обязана быть в списке");

    let one = app.get_auth(&format!("/api/estimates/{id}"), &token).await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(one.body["id"], id);
}

#[tokio::test]
async fn upload_and_list_require_login() {
    let app = spawn_app().await;
    let res = app
        .post_file("/api/estimates", "smeta.xlsx", b"x", "не-токен")
        .await;
    assert_eq!(res.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.get("/api/estimates").await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn foreign_and_missing_estimates_look_the_same() {
    let app = spawn_app().await;
    let (alice, _) = app.register_user().await;
    let (bob, _) = app.register_user().await;

    let created = app
        .post_file("/api/estimates", "smeta.xlsx", b"alice", &alice)
        .await;
    let id = created.body["id"].as_str().expect("id сметы").to_owned();

    let foreign = app.get_auth(&format!("/api/estimates/{id}"), &bob).await;
    let missing = app
        .get_auth("/api/estimates/00000000-0000-0000-0000-000000000000", &bob)
        .await;
    assert_eq!(foreign.status, StatusCode::NOT_FOUND);
    assert_eq!(missing.status, foreign.status);
    assert_eq!(missing.body, foreign.body, "ответы обязаны быть неотличимы");

    // и в списке Боба чужой сметы нет
    let list = app.get_auth("/api/estimates", &bob).await;
    assert_eq!(list.body.as_array().expect("список").len(), 0);
}

#[tokio::test]
async fn unsupported_format_is_rejected() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;

    let pdf = app
        .post_file("/api/estimates", "smeta.pdf", b"%PDF-1.4", &token)
        .await;
    assert_eq!(pdf.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(pdf.body["error"]["code"], "error-estimate-format");
    assert_eq!(
        pdf.body["error"]["fields"][0]["field"], "file",
        "ошибка обязана указывать на поле: {}",
        pdf.body
    );
    let message = pdf.body["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("Excel"), "текст локализован: {message}");

    let empty = app
        .post_file("/api/estimates", "smeta.xlsx", b"", &token)
        .await;
    assert_eq!(empty.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(empty.body["error"]["code"], "error-estimate-empty");
}

/// Минимальный jpeg: до разбора дело в этих тестах не доходит, а на загрузке
/// проверяются ровно первые байты
const JPEG: &[u8] = b"\xff\xd8\xff\xe0\x00\x10JFIF photo of an estimate";

/// Приложение с включённой нейросетью (адрес провайдера тут не важен: до
/// вызова дело доходит только в разборе)
async fn app_with_llm() -> crate::common::TestApp {
    spawn_app_with(|settings| settings.llm.api_key = Some("test-llm-key".to_owned().into())).await
}

#[tokio::test]
async fn photo_is_accepted_from_a_user_with_confirmed_email() {
    let app = app_with_llm().await;
    let (token, _) = app.register_verified_user().await;

    let res = app
        .post_file("/api/estimates", "Смета.jpg", JPEG, &token)
        .await;

    assert_eq!(res.status, StatusCode::CREATED, "ответ: {}", res.body);
    assert_eq!(
        res.body["from_photo"], true,
        "смета обязана знать, что она с фото"
    );
    let id = res.body["id"].as_str().expect("id сметы").to_owned();
    assert!(
        app.files_dir.join(format!("{id}.jpg")).exists(),
        "фото не легло на диск"
    );
    // у Excel-сметы того же пользователя признак остаётся выключенным
    let excel = app
        .post_file("/api/estimates", "Смета.xlsx", &fixture(SMALL_XLSX), &token)
        .await;
    assert_eq!(excel.body["from_photo"], false);
}

#[tokio::test]
async fn photo_without_confirmed_email_is_refused_with_a_readable_reason() {
    let app = app_with_llm().await;
    let (token, _) = app.register_user().await;

    let res = app
        .post_file("/api/estimates", "Смета.jpg", JPEG, &token)
        .await;

    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        res.body["error"]["code"],
        "error-estimate-photo-needs-email"
    );
    // Excel-путь подтверждения почты не требует
    let excel = app
        .post_file("/api/estimates", "Смета.xlsx", &fixture(SMALL_XLSX), &token)
        .await;
    assert_eq!(excel.status, StatusCode::CREATED);
}

#[tokio::test]
async fn photo_is_refused_while_the_neural_network_is_off() {
    let app = spawn_app().await;
    let (token, _) = app.register_verified_user().await;

    let res = app
        .post_file("/api/estimates", "Смета.jpg", JPEG, &token)
        .await;

    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["error"]["code"], "error-estimate-photo-off");
}

#[tokio::test]
async fn garbage_renamed_to_jpg_is_rejected_before_any_recognition() {
    let app = app_with_llm().await;
    let (token, _) = app.register_verified_user().await;

    let res = app
        .post_file(
            "/api/estimates",
            "Смета.jpg",
            b"PK\x03\x04 zip archive",
            &token,
        )
        .await;

    assert_eq!(res.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(res.body["error"]["code"], "error-estimate-not-a-photo");
    let saved: i64 = sqlx::query_scalar("SELECT count(*) FROM estimates")
        .fetch_one(&app.pool)
        .await
        .expect("счёт смет");
    assert_eq!(saved, 0, "мусор не должен становиться сметой");
}

#[tokio::test]
async fn file_over_the_limit_is_rejected_with_a_readable_reason() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    // чуть больше потолка файла, но в пределах потолка тела: пользователь
    // обязан получить объяснение, а не голый 413 от фреймворка
    let huge = vec![b'x'; server::estimates::MAX_FILE_BYTES + 8 * 1024];

    let res = app
        .post_file("/api/estimates", "smeta.xlsx", &huge, &token)
        .await;
    assert_eq!(res.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(res.body["error"]["code"], "error-estimate-too-large");
    assert!(
        res.body["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("10")),
        "в тексте обязан быть потолок: {}",
        res.body
    );
}

#[tokio::test]
async fn upload_is_rate_limited_per_ip() {
    let app = spawn_app_with(|s| s.rate_limit_upload_rpm = 2).await;
    let (token, _) = app.register_user().await;

    for n in 0..2 {
        let res = app
            .post_file("/api/estimates", "smeta.xlsx", b"x", &token)
            .await;
        assert_eq!(
            res.status,
            StatusCode::CREATED,
            "загрузка {n}: {}",
            res.body
        );
    }
    let res = app
        .post_file("/api/estimates", "smeta.xlsx", b"x", &token)
        .await;
    assert_eq!(res.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.body["error"]["code"], "error-too-many-requests");
}

#[tokio::test]
async fn estimates_per_user_are_capped() {
    let app = spawn_app().await;
    let (token, _) = app.register_user().await;
    for n in 0..server::estimates::MAX_PER_USER {
        let res = app
            .post_file("/api/estimates", "smeta.xlsx", b"x", &token)
            .await;
        assert_eq!(
            res.status,
            StatusCode::CREATED,
            "загрузка {n}: {}",
            res.body
        );
    }

    let over = app
        .post_file("/api/estimates", "smeta.xlsx", b"x", &token)
        .await;
    assert_eq!(over.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(over.body["error"]["code"], "error-estimate-limit");
}
