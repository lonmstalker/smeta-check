//! Тест-инфраструктура бэкенда. Цели: ноль дублирования в тестах,
//! минимум ресурсов, гарантированная очистка.
//!
//! База данных, два пути:
//! - Быстрый: задан TEST_PG_URL (например, база из `make dev`) — каждому
//!   тесту создаётся своя БД за миллисекунды, контейнеры не поднимаются.
//!   `make test` использует именно этот путь.
//! - Автономный: без TEST_PG_URL на каждый тест поднимается свой
//!   testcontainer; Drop контейнера гарантирует очистку (`docker ps` пуст).

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::core::Mount;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tower::ServiceExt;

use server::core::config::Settings;

/// Пароль всех тестовых пользователей (проходит валидацию длины)
pub const PASSWORD: &str = "correct-horse-9";

pub struct TestApp {
    pub router: Router,
    pub pool: PgPool,
    /// каталог файлов этого теста; удаляется вместе с TestApp
    pub files_dir: std::path::PathBuf,
    _container: Option<ContainerAsync<Postgres>>,
    _files: tempfile::TempDir,
}

/// Тестовая смета из `server/tests/fixtures/estimates` — настоящие файлы
/// из открытых источников (README рядом с ними)
pub fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/estimates")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|err| panic!("нет фикстуры {path:?}: {err}"))
}

/// Приложение на чистой БД — одна строка в начале каждого теста
pub async fn spawn_app() -> TestApp {
    spawn_app_with(|_| {}).await
}

/// То же, но с подкрученной конфигурацией: окружение процесса не трогаем,
/// поэтому тесты не мешают друг другу.
pub async fn spawn_app_with(tune: impl FnOnce(&mut Settings)) -> TestApp {
    let mut settings = Settings::for_tests();
    // файлы теста живут в своём временном каталоге: TempDir сотрёт его сам
    let files = tempfile::TempDir::new().expect("temp dir for files");
    settings.files_dir = files.path().to_path_buf();
    tune(&mut settings);
    let (pool, container) = match std::env::var("TEST_PG_URL").ok().filter(|u| !u.is_empty()) {
        Some(url) => (fresh_db_on_server(&url).await, None),
        None => {
            let (pool, container) = fresh_container().await;
            (pool, Some(container))
        }
    };
    server::core::db::MIGRATIONS
        .run(&pool)
        .await
        .expect("migrations apply");
    TestApp {
        router: server::app(server::AppState {
            pool: pool.clone(),
            settings: Arc::new(settings),
        }),
        pool,
        files_dir: files.path().to_path_buf(),
        _container: container,
        _files: files,
    }
}

async fn fresh_db_on_server(url: &str) -> PgPool {
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(url)
        .await
        .expect("connect TEST_PG_URL (docker compose up -d db?)");
    let name = format!("test_{}", uuid::Uuid::new_v4().simple());
    // имя генерируем сами (hex uuid) — инъекция невозможна
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&admin)
        .await
        .expect("create test database");
    // ponytail: базы test_* копятся до перезапуска compose (tmpfs всё стирает);
    // если станет мешать — дропать старые здесь же одним DROP по списку
    let base = url.rsplit_once('/').expect("url with db path").0;
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&format!("{base}/{name}"))
        .await
        .expect("connect test database")
}

async fn fresh_container() -> (PgPool, ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .with_tag("18-alpine")
        // с postgres:18 данные лежат в /var/lib/postgresql (не .../data)
        .with_mount(Mount::tmpfs_mount("/var/lib/postgresql"))
        .start()
        .await
        .expect("start postgres container (docker running?)");
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .unwrap();
    (pool, container)
}

// --- компактный HTTP-клиент для тестов --------------------------------------

pub struct TestResponse {
    pub status: StatusCode,
    pub body: Value,
    /// значения Set-Cookie — для проверок refresh-токена
    pub cookies: Vec<String>,
    /// все заголовки ответа — для проверок CORS и кэширования
    pub headers: HeaderMap,
}

impl TestApp {
    pub async fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&Value>,
        token: Option<&str>,
        extra_headers: &[(&str, &str)],
    ) -> TestResponse {
        let mut builder = Request::builder().method(method).uri(path);
        if body.is_some() {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        if let Some(token) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        for (name, value) in extra_headers {
            builder = builder.header(*name, *value);
        }
        let request = builder
            .body(body.map_or(Body::empty(), |b| Body::from(b.to_string())))
            .unwrap();
        self.send(request).await
    }

    /// Загрузка файла: тело multipart собирается руками — ради двух границ и
    /// одного заголовка тащить крейт-построитель незачем
    pub async fn post_file(
        &self,
        path: &str,
        file_name: &str,
        content: &[u8],
        token: &str,
    ) -> TestResponse {
        const BOUNDARY: &str = "smeta-check-test-boundary";
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(content);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        let request = Request::builder()
            .method("POST")
            .uri(path)
            .header(
                header::CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(body))
            .unwrap();
        self.send(request).await
    }

    async fn send(&self, request: Request<Body>) -> TestResponse {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|v| v.to_str().unwrap_or_default().to_owned())
            .collect();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        TestResponse {
            status,
            body,
            cookies,
            headers,
        }
    }

    pub async fn get(&self, path: &str) -> TestResponse {
        self.request("GET", path, None, None, &[]).await
    }

    pub async fn get_auth(&self, path: &str, token: &str) -> TestResponse {
        self.request("GET", path, None, Some(token), &[]).await
    }

    pub async fn post(&self, path: &str, body: Value) -> TestResponse {
        self.request("POST", path, Some(&body), None, &[]).await
    }

    pub async fn patch_auth(&self, path: &str, body: Value, token: &str) -> TestResponse {
        self.request("PATCH", path, Some(&body), Some(token), &[])
            .await
    }

    pub async fn post_auth(&self, path: &str, body: Value, token: &str) -> TestResponse {
        self.request("POST", path, Some(&body), Some(token), &[])
            .await
    }

    pub async fn delete_auth(&self, path: &str, token: &str) -> TestResponse {
        self.request("DELETE", path, None, Some(token), &[]).await
    }

    /// Регистрация свежего пользователя; возвращает access-токен и email
    pub async fn register_user(&self) -> (String, String) {
        let email = format!("user-{}@test.local", uuid::Uuid::new_v4().simple());
        let res = self
            .post(
                "/api/auth/register",
                json!({ "email": email, "password": PASSWORD }),
            )
            .await;
        assert_eq!(
            res.status,
            StatusCode::CREATED,
            "register failed: {}",
            res.body
        );
        (res.body["access_token"].as_str().unwrap().to_owned(), email)
    }

    /// Сделать пользователя администратором (только для тестов)
    pub async fn promote_to_admin(&self, email: &str) {
        sqlx::query("UPDATE users SET role = 'admin' WHERE email = $1")
            .bind(email)
            .execute(&self.pool)
            .await
            .expect("promote to admin");
    }

    /// Достать «сырой» refresh-токен из Set-Cookie ответа
    pub fn refresh_token_of(res: &TestResponse) -> Option<String> {
        res.cookies
            .iter()
            .find(|c| c.starts_with("refresh_token="))
            .and_then(|c| c.split(['=', ';']).nth(1))
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    }

    /// Письмо из dev-ящика (outbox) — например, для чтения ссылки сброса
    pub async fn last_email_to(&self, recipient: &str) -> Option<String> {
        sqlx::query_scalar(
            "SELECT body FROM outbox_emails WHERE recipient = $1 ORDER BY id DESC LIMIT 1",
        )
        .bind(recipient)
        .fetch_optional(&self.pool)
        .await
        .expect("read outbox")
    }
}
