//! Разбор сметы с фотографии: нейросеть переписывает лист, мы строго
//! разбираем её ответ.
//!
//! Главное правило здесь — неверное число хуже отсутствующего. Поэтому
//! промпт запрещает вычислять то, чего на листе нет, а ответ проверяется у
//! нас: строка, у которой количество × цена не сходится с суммой, теряет
//! числа и уходит в нераспознанные (UC-003.5), а не показывается человеку
//! как факт. Фотография — недоверенный вход: текст с неё никогда не
//! исполняется, только раскладывается по полям.

use serde::Deserialize;
use sqlx::PgPool;

use crate::core::config::Settings;
use crate::core::llm::{self, Image, LlmError};
use crate::estimates::parse::ParsedLine;

/// «Лист» у фотографии один — подписываем строки одинаково
const SHEET: &str = "Фото";

/// Допуск сверки «количество × цена = сумма»: округления в смете обычны,
/// расхождение больше процента — это уже другое число.
const TOTAL_TOLERANCE: f64 = 0.01;

/// Тот же промпт, что и в замере качества (`scripts/llm-probe.mjs`): планка
/// F0 измерена именно на нём, поэтому меняешь здесь — перемеряй там.
const PROMPT: &str = r#"Ты переписываешь смету на ремонт с фотографии в JSON.

Верни ТОЛЬКО JSON без пояснений и без markdown:
{"lines":[{"name":"…","unit":"…","quantity":0,"price":0,"total":0}],"unreadable":["…"]}

Правила:
- Переписывай строки как видишь. Не восстанавливай и не вычисляй числа,
  которых нет на листе: нет числа — не ставь поле.
- Ничего не придумывай: каждое число должно быть видно на фотографии.
- Заголовки разделов («Стены», «Потолок») тоже строки — только с именем.
- Неразборчивое верни в "unreadable" как есть, а не догадкой."#;

/// Почему не получилось. Разделение то же, что у всей волны: виноват кадр
/// или виноват провайдер — от этого зависит, тратится ли попытка разбора.
#[derive(Debug)]
pub enum PhotoError {
    /// модель ответила, но сметы из ответа не собрать — дело в кадре
    BadAnswer,
    /// до модели не дошли, она недоступна или выбран потолок токенов
    Provider(LlmError),
}

/// Разобрать фотографию сметы в строки
pub async fn parse(
    pool: &PgPool,
    settings: &Settings,
    bytes: &[u8],
    ext: &str,
) -> Result<Vec<ParsedLine>, PhotoError> {
    let image = Image {
        bytes,
        mime: mime_of(ext),
    };
    let text = llm::complete(pool, settings, PROMPT, &[image])
        .await
        .map_err(PhotoError::Provider)?;
    let answer = answer_of(&text).ok_or_else(|| {
        tracing::warn!(
            answer = text.chars().take(300).collect::<String>(),
            "ответ нейросети не разобрать как JSON сметы"
        );
        PhotoError::BadAnswer
    })?;
    let lines = to_lines(answer);
    if lines.is_empty() {
        return Err(PhotoError::BadAnswer);
    }
    Ok(lines)
}

/// Тип картинки для провайдера — по расширению, которое мы сами и выдали
fn mime_of(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    lines: Vec<AnswerLine>,
    /// то, что модель не смогла прочитать: показываем как есть
    #[serde(default)]
    unreadable: Vec<String>,
}

#[derive(Deserialize)]
struct AnswerLine {
    #[serde(default)]
    name: String,
    unit: Option<String>,
    quantity: Option<f64>,
    price: Option<f64>,
    total: Option<f64>,
}

/// Достать JSON из ответа модели. Модели любят обернуть его в ```json и
/// добавить вежливое предисловие — снимаем обёртку, дальше строго.
fn answer_of(text: &str) -> Option<Answer> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    serde_json::from_str(text.get(start..=end)?).ok()
}

/// Превратить ответ в строки сметы. Ничего не выбрасываем: не сошедшееся
/// число — повод спрятать число, а не строку.
fn to_lines(answer: Answer) -> Vec<ParsedLine> {
    let mut lines = Vec::new();
    for line in answer.lines {
        let title = line.name.trim().to_owned();
        if title.is_empty() {
            continue;
        }
        let quantity = usable(line.quantity);
        let price = usable(line.price);
        let total = usable(line.total);
        let unit = line
            .unit
            .map(|u| u.trim().to_owned())
            .filter(|u| !u.is_empty());
        let raw_text = raw_text_of(&title, unit.as_deref(), quantity, price, total);
        if numbers_agree(quantity, price, total) {
            lines.push(ParsedLine {
                sheet: SHEET.into(),
                raw_text,
                title: Some(title),
                unit,
                quantity,
                price,
                total,
            });
        } else {
            // числа спорят между собой — показываем строку как есть и не
            // выдаём ни одно из них за правду
            tracing::info!(line = raw_text, "числа строки не сошлись, ушла в сырые");
            lines.push(ParsedLine {
                sheet: SHEET.into(),
                raw_text,
                ..ParsedLine::default()
            });
        }
    }
    for text in answer.unreadable {
        let text = text.trim();
        if !text.is_empty() {
            lines.push(ParsedLine {
                sheet: SHEET.into(),
                raw_text: text.to_owned(),
                ..ParsedLine::default()
            });
        }
    }
    lines
}

/// Ноль и отрицательное в смете — это не число, а пустая клетка, которую
/// модель зачем-то заполнила
fn usable(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v > 0.0)
}

/// Сверка чисел строки: есть все три — обязаны сходиться
fn numbers_agree(quantity: Option<f64>, price: Option<f64>, total: Option<f64>) -> bool {
    let (Some(quantity), Some(price), Some(total)) = (quantity, price, total) else {
        return true;
    };
    let expected = quantity * price;
    (expected - total).abs() <= (total.abs() * TOTAL_TOLERANCE).max(TOTAL_TOLERANCE)
}

/// Строка глазами человека — то же, что видно на листе
fn raw_text_of(
    title: &str,
    unit: Option<&str>,
    quantity: Option<f64>,
    price: Option<f64>,
    total: Option<f64>,
) -> String {
    let mut parts = vec![title.to_owned()];
    if let Some(unit) = unit {
        parts.push(unit.to_owned());
    }
    for number in [quantity, price, total].into_iter().flatten() {
        parts.push(format!("{number}"));
    }
    parts.join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(json: &str) -> Vec<ParsedLine> {
        to_lines(answer_of(json).expect("ответ разобран"))
    }

    #[test]
    fn answer_in_markdown_fence_is_still_read() {
        let lines = lines_of(
            "Вот смета:\n```json\n{\"lines\":[{\"name\":\"Штукатурка стен\",\
             \"unit\":\"м2\",\"quantity\":10,\"price\":350,\"total\":3500}]}\n```",
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].title.as_deref(), Some("Штукатурка стен"));
        assert_eq!(lines[0].total, Some(3500.0));
    }

    #[test]
    fn line_whose_numbers_disagree_keeps_text_and_loses_numbers() {
        let lines = lines_of(
            "{\"lines\":[{\"name\":\"Стяжка пола\",\"quantity\":10,\"price\":350,\"total\":9999}]}",
        );
        assert_eq!(lines.len(), 1, "строку выбрасывать нельзя");
        assert_eq!(lines[0].title, None, "числа спорят — строка не распознана");
        assert_eq!(lines[0].price, None);
        assert!(lines[0].raw_text.contains("Стяжка пола"));
    }

    #[test]
    fn zeroes_in_empty_cells_do_not_become_numbers() {
        let lines =
            lines_of("{\"lines\":[{\"name\":\"СТЕНЫ\",\"quantity\":0,\"price\":0,\"total\":0}]}");
        assert_eq!(lines[0].title.as_deref(), Some("СТЕНЫ"));
        assert_eq!(lines[0].quantity, None);
        assert!(!lines[0].is_recognized(), "заголовок раздела — не позиция");
    }

    #[test]
    fn unreadable_pieces_are_kept_as_raw_lines() {
        let lines = lines_of("{\"lines\":[],\"unreadable\":[\"нижняя строка смазана\"]}");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].raw_text, "нижняя строка смазана");
        assert_eq!(lines[0].title, None);
    }

    #[test]
    fn text_without_json_is_a_bad_answer() {
        assert!(answer_of("извините, я не вижу смету на этой фотографии").is_none());
    }
}
