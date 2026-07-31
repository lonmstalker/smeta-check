//! Вызов нейросети через OpenAI-совместимый API: текст плюс картинки.
//!
//! Абстракции «провайдер» здесь нет намеренно: сам протокол
//! `/chat/completions` и есть интерфейс, а смена провайдера — три переменные
//! окружения (`docs/llm.md`). Провайдероспецифичных полей не используем, иначе
//! переезд перестанет быть бесплатным.
//!
//! Предохранители: ключа нет — интеграция выключена; за сутки тратим не
//! больше `DAILY_TOKENS_CAP` токенов; в базу пишем только счётчики (ни фото,
//! ни ответов), поэтому утекать из этой таблицы нечему.

use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use sqlx::PgPool;

use crate::core::config::Settings;

/// Дневной потолок токенов на весь сервис (UTC-сутки). При замере F0 одно
/// фото сметы стоило около 2000 токенов — это порядка тысячи фото в день.
/// Потолок один на всех: персональной квоты нет, её роль выполняют потолок
/// смет на аккаунт и требование подтверждённой почты.
const DAILY_TOKENS_CAP: i64 = 2_000_000;

/// Потолок ответа модели. Замер F0: ответ на смету в 40 строк — 1351 токен,
/// значит смета на 80 строк укладывается с запасом.
const MAX_TOKENS: u32 = 4000;

/// Дольше ждать бессмысленно: разбор фоновый, попытку повторит воркер.
const TIMEOUT: Duration = Duration::from_secs(60);

/// Сколько дней держим счётчики вызовов — дальше это балласт
const KEEP_CALLS_DAYS: i32 = 30;

/// Почему вызов не состоялся. Общее у всех вариантов одно: виноват не
/// пользователь и не его файл, поэтому попытку разбора они не тратят
/// (см. решение 7 плана волны).
#[derive(Debug)]
pub enum LlmError {
    /// ключ не задан — нейросеть выключена
    Disabled,
    /// дневной потолок токенов выбран, до завтра не зовём
    Capped,
    /// провайдер ответил не 2xx: неверный ключ, лимит провайдера, его авария
    Http(u16),
    /// ответа не дождались
    Timeout,
    /// не дошли до модели или ответ нечитаем (сеть, база, битый JSON)
    Transport(String),
}

/// Картинка для модели: байты как есть плюс их тип. Base64 делаем здесь —
/// снаружи о формате запроса знать не нужно.
pub struct Image<'a> {
    pub bytes: &'a [u8],
    pub mime: &'a str,
}

/// Включена ли нейросеть. Без ключа дорогие входы (фото) не принимаются, а
/// остальной сервис работает как раньше.
pub fn enabled(settings: &Settings) -> bool {
    settings.llm.api_key.is_some()
}

/// Спросить модель и вернуть текст ответа как есть — разбирает его тот, кто
/// звал: у каждого потребителя свой формат и своя строгость.
pub async fn complete(
    pool: &PgPool,
    settings: &Settings,
    prompt: &str,
    images: &[Image<'_>],
) -> Result<String, LlmError> {
    let Some(key) = &settings.llm.api_key else {
        return Err(LlmError::Disabled);
    };
    let spent = spent_today(pool)
        .await
        .map_err(|err| LlmError::Transport(format!("учёт токенов недоступен: {err}")))?;
    if spent >= DAILY_TOKENS_CAP {
        metrics::counter!("llm_calls_total", "result" => "capped").increment(1);
        tracing::warn!(
            spent,
            cap = DAILY_TOKENS_CAP,
            "дневной потолок токенов выбран"
        );
        return Err(LlmError::Capped);
    }

    // клиент на вызов: вызовов немного и они долгие, экономить нечего
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|err| LlmError::Transport(err.to_string()))?;
    let url = format!(
        "{}/chat/completions",
        settings.llm.base_url.trim_end_matches('/')
    );
    let response = client
        .post(url)
        // ключ живёт только в заголовке: ни в теле, ни в логах его нет
        .bearer_auth(key.expose())
        .json(&body(&settings.llm.model, prompt, images))
        .send()
        .await
        .map_err(|err| {
            if err.is_timeout() {
                metrics::counter!("llm_calls_total", "result" => "timeout").increment(1);
                LlmError::Timeout
            } else {
                metrics::counter!("llm_calls_total", "result" => "transport").increment(1);
                LlmError::Transport(err.to_string())
            }
        })?;

    let status = response.status();
    if !status.is_success() {
        metrics::counter!("llm_calls_total", "result" => "http_error").increment(1);
        // тело ошибки провайдера — в лог оператору, наружу оно не идёт
        let details = response.text().await.unwrap_or_default();
        tracing::error!(
            status = status.as_u16(),
            details = details.chars().take(500).collect::<String>(),
            "провайдер нейросети ответил ошибкой"
        );
        return Err(LlmError::Http(status.as_u16()));
    }
    let answer: Answer = response
        .json()
        .await
        .map_err(|err| LlmError::Transport(format!("ответ провайдера нечитаем: {err}")))?;

    record(pool, &settings.llm.model, &answer.usage).await;
    metrics::counter!("llm_calls_total", "result" => "ok").increment(1);
    metrics::counter!("llm_tokens_total", "kind" => "prompt")
        .increment(answer.usage.prompt_tokens.max(0) as u64);
    metrics::counter!("llm_tokens_total", "kind" => "completion")
        .increment(answer.usage.completion_tokens.max(0) as u64);
    Ok(answer
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .unwrap_or_default())
}

/// Тело запроса в формате OpenAI: одно сообщение пользователя, в нём текст и
/// картинки. Картинки уходят инлайном (data:), чтобы не заводить ни хранилища
/// с публичными ссылками, ни срока жизни у них.
fn body(model: &str, prompt: &str, images: &[Image<'_>]) -> serde_json::Value {
    let mut content = vec![serde_json::json!({ "type": "text", "text": prompt })];
    for image in images {
        let data = base64::engine::general_purpose::STANDARD.encode(image.bytes);
        content.push(serde_json::json!({
            "type": "image_url",
            "image_url": { "url": format!("data:{};base64,{}", image.mime, data) },
        }));
    }
    serde_json::json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "messages": [{ "role": "user", "content": content }],
    })
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Usage,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: String,
}

/// Провайдер может не прислать учёт — тогда считаем вызов бесплатным: потолок
/// страхует порядок величины, а не бухгалтерию.
#[derive(Deserialize, Default)]
struct Usage {
    #[serde(default)]
    prompt_tokens: i32,
    #[serde(default)]
    completion_tokens: i32,
}

/// Сколько токенов потрачено с начала суток по UTC
async fn spent_today(pool: &PgPool) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "SELECT coalesce(sum(prompt_tokens + completion_tokens), 0)::bigint FROM llm_calls
         WHERE created_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC'",
    )
    .fetch_one(pool)
    .await
}

/// Записать потраченное. Не записалось — вызов уже состоялся, поэтому только
/// лог: терять готовый ответ из-за счётчика глупо.
async fn record(pool: &PgPool, model: &str, usage: &Usage) {
    let saved = sqlx::query(
        "INSERT INTO llm_calls (model, prompt_tokens, completion_tokens) VALUES ($1, $2, $3)",
    )
    .bind(model)
    .bind(usage.prompt_tokens.max(0))
    .bind(usage.completion_tokens.max(0))
    .execute(pool)
    .await;
    if let Err(err) = saved {
        tracing::error!(error = %err, "учёт вызова нейросети не записался");
    }
}

/// Выбросить старые счётчики: для потолка нужны только сегодняшние
pub async fn cleanup_old_calls(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM llm_calls WHERE created_at < now() - make_interval(days => $1)")
        .bind(KEEP_CALLS_DAYS)
        .execute(pool)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_carries_prompt_and_image() {
        let image = Image {
            bytes: b"\xff\xd8\xff",
            mime: "image/jpeg",
        };
        let body = body("some/model", "перепиши смету", &[image]);
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["text"], "перепиши смету");
        let url = content[1]["image_url"]["url"].as_str().unwrap_or_default();
        assert!(
            url.starts_with("data:image/jpeg;base64,"),
            "картинка ушла не как data-url: {url}"
        );
        assert_eq!(body["max_tokens"], MAX_TOKENS);
    }
}
