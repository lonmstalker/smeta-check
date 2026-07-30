//! Контракт API не может «уехать» незаметно: спека в репозитории обязана
//! совпадать с кодом, а валидации в спеке — с константами кода.
//! Обновление спеки: make gen-api (перегенерирует и typescript-типы фронта).

use utoipa::OpenApi;

fn spec_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../openapi.json")
}

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "хелпер теста: паника валит тест — это и есть отчёт об ошибке"
)]
fn current_spec() -> String {
    let mut json = server::ApiDoc::openapi()
        .to_pretty_json()
        .expect("spec serializes");
    json.push('\n');
    json
}

#[test]
fn openapi_spec_is_committed_and_current() {
    let current = current_spec();
    if std::env::var("UPDATE_OPENAPI").is_ok() {
        std::fs::write(spec_path(), &current).expect("write openapi.json");
        return;
    }
    let committed =
        std::fs::read_to_string(spec_path()).expect("openapi.json missing — run `make gen-api`");
    assert_eq!(
        committed, current,
        "openapi.json устарел: запусти `make gen-api` и закоммить изменения"
    );
}

#[test]
fn openapi_validations_match_code_constants() {
    let spec: serde_json::Value = serde_json::from_str(&current_spec()).expect("valid json");
    let min = spec
        .pointer("/components/schemas/Credentials/properties/password/minLength")
        .and_then(serde_json::Value::as_u64)
        .expect("password minLength in spec");
    assert_eq!(
        min as usize,
        server::auth::password::MIN_PASSWORD_LEN,
        "минимальная длина пароля в OpenAPI разошлась с кодом"
    );
}
