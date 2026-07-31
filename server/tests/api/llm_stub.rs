//! Фейковый провайдер нейросети: настоящий HTTP-сервер на случайном порту с
//! заданным ответом. Нужен, потому что клиент ходит по сети — подменять
//! reqwest нечем, а настоящий провайдер в тестах означал бы деньги, интернет
//! и разные ответы на один и тот же запрос.
//!
//! Сервер живёт до конца тестового бинаря: слушатель на порту 0 и одна задача
//! tokio, гасить их отдельно незачем.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::routing::post;

/// Сколько токенов «потратил» фейковый вызов — тесты учёта сверяют эти числа
pub const STUB_PROMPT_TOKENS: i32 = 1000;
pub const STUB_COMPLETION_TOKENS: i32 = 500;

pub struct LlmStub {
    /// адрес для `Settings::llm.base_url`
    pub base_url: String,
    calls: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct StubState {
    calls: Arc<AtomicUsize>,
    status: StatusCode,
    body: Arc<String>,
}

impl LlmStub {
    /// Провайдер, который отвечает моделью с готовым текстом
    pub async fn answering(content: &str) -> Self {
        Self::start(StatusCode::OK, chat_body(content)).await
    }

    /// Провайдер, который отвечает ошибкой: неверный ключ, авария, лимит
    pub async fn failing(status: StatusCode) -> Self {
        Self::start(status, r#"{"error":"провайдер недоступен"}"#.to_owned()).await
    }

    /// Сколько раз к нему обратились: главный вопрос теста про потолок —
    /// «а в сеть-то не пошли?»
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::Relaxed)
    }

    async fn start(status: StatusCode, body: String) -> Self {
        let calls = Arc::new(AtomicUsize::new(0));
        let state = StubState {
            calls: calls.clone(),
            status,
            body: Arc::new(body),
        };
        let app = Router::new()
            .route("/chat/completions", post(reply))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("порт для фейкового провайдера");
        let addr = listener.local_addr().expect("адрес фейкового провайдера");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self {
            base_url: format!("http://{addr}"),
            calls,
        }
    }
}

async fn reply(
    State(state): State<StubState>,
) -> (StatusCode, [(header::HeaderName, &'static str); 1], String) {
    state.calls.fetch_add(1, Ordering::Relaxed);
    (
        state.status,
        [(header::CONTENT_TYPE, "application/json")],
        state.body.to_string(),
    )
}

/// Ответ в формате OpenAI: тот же, что придёт от настоящего провайдера
fn chat_body(content: &str) -> String {
    serde_json::json!({
        "choices": [{ "message": { "role": "assistant", "content": content } }],
        "usage": {
            "prompt_tokens": STUB_PROMPT_TOKENS,
            "completion_tokens": STUB_COMPLETION_TOKENS,
        },
    })
    .to_string()
}
