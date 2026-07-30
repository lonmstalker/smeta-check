//! Разбор файла сметы в строки.
//!
//! Парсер мягкий: файл прочитался — смета разобрана. Строку, которую не
//! удалось понять, мы не выбрасываем и не считаем ошибкой, а сохраняем сырым
//! текстом: человек увидит её в блоке «спросите бригаду, что это».
//! `failed` бывает только у файла, который вообще не читается.
//!
//! Модуль разбит по смыслу: `book` — чтение книги, `header` — поиск шапки и
//! колонок, `line` — разбор одной строки.

mod book;
mod header;
mod line;

/// Строка сметы после разбора. Распознанные поля могут быть пустыми — тогда
/// у строки остаётся только сырой текст.
#[derive(Debug, Default, PartialEq)]
pub struct ParsedLine {
    pub sheet: String,
    pub raw_text: String,
    pub title: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub total: Option<f64>,
}

impl ParsedLine {
    /// Строка считается распознанной, когда у неё есть название и хотя бы
    /// одно число: без числа это просто заголовок раздела.
    pub fn is_recognized(&self) -> bool {
        self.title.is_some()
            && (self.quantity.is_some() || self.price.is_some() || self.total.is_some())
    }
}

/// Почему файл не удалось прочитать. Каждый вариант — ключ локализации:
/// текст подставляется на языке пользователя при отдаче.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// файл не открывается как книга Excel
    Unreadable,
    /// книга открылась, но в ней нет ни одной непустой строки
    NoData,
    /// книга слишком большая: столько листов и строк смета не бывает
    TooBig,
}

impl ParseError {
    pub fn key(self) -> &'static str {
        match self {
            ParseError::Unreadable => "error-estimate-unreadable",
            ParseError::NoData => "error-estimate-no-data",
            ParseError::TooBig => "error-estimate-too-big",
        }
    }

    /// короткое имя для метки метрики (в метки не кладут длинные ключи)
    pub fn reason(self) -> &'static str {
        match self {
            ParseError::Unreadable => "unreadable",
            ParseError::NoData => "no_data",
            ParseError::TooBig => "too_big",
        }
    }
}

/// Потолки распаковки: xlsx — это zip, и пять мегабайт файла разворачиваются
/// в гигабайты, если внутри лежит лист на миллион строк.
const MAX_SHEETS: usize = 20;
const MAX_ROWS_PER_SHEET: usize = 20_000;
const MAX_COLUMNS: usize = 100;
/// Больше строк в смете не бывает; хвост отбрасываем, чтобы не залить базу
pub const MAX_LINES: usize = 5_000;

/// Разобрать файл сметы. Функция синхронная и тяжёлая — звать её только из
/// `spawn_blocking`, иначе она заблокирует поток исполнителя tokio.
pub fn parse(bytes: Vec<u8>) -> Result<Vec<ParsedLine>, ParseError> {
    let sheets = book::read(bytes)?;
    let mut lines = Vec::new();
    for sheet in sheets {
        let columns = header::find(&sheet.rows);
        let start = columns.as_ref().map_or(0, |c| c.header_row + 1);
        for row in sheet.rows.iter().skip(start) {
            if lines.len() >= MAX_LINES {
                tracing::warn!(limit = MAX_LINES, "в смете больше строк, чем мы храним");
                return finish(lines);
            }
            if let Some(parsed) = line::parse(&sheet.name, row, columns.as_ref()) {
                lines.push(parsed);
            }
        }
    }
    finish(lines)
}

fn finish(lines: Vec<ParsedLine>) -> Result<Vec<ParsedLine>, ParseError> {
    if lines.is_empty() {
        return Err(ParseError::NoData);
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_without_numbers_is_not_recognized() {
        let section = ParsedLine {
            title: Some("СТЕНЫ".into()),
            ..ParsedLine::default()
        };
        assert!(!section.is_recognized());

        let work = ParsedLine {
            title: Some("Штукатурка стен".into()),
            quantity: Some(12.0),
            ..ParsedLine::default()
        };
        assert!(work.is_recognized());
    }

    #[test]
    fn empty_book_is_an_error_not_an_empty_estimate() {
        assert_eq!(finish(Vec::new()).unwrap_err(), ParseError::NoData);
    }
}
