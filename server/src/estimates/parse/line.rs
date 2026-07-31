//! Разбор одной строки листа.
//!
//! Правило честности: если шапки нет, мы НЕ угадываем, где цена, а где
//! количество — неверная цена хуже отсутствующей. Тогда у строки остаётся
//! название и итоговое число, остальное честно пустое.

use super::ParsedLine;
use super::book::Cell;
use super::header::Columns;

/// Сырой текст строки не должен превращаться в простыню
const MAX_RAW_LEN: usize = 500;

/// Единица измерения — это «кв.м.», а не абзац текста
const MAX_UNIT_LEN: usize = 20;

/// Короткое слово из одной-двух букв названием работы быть не может
const MIN_TITLE_LEN: usize = 3;

const UNITS: [&str; 16] = [
    "кв.м.",
    "кв.м",
    "м2",
    "м²",
    "пог.м",
    "п.м",
    "м.п.",
    "м/п",
    "м3",
    "м³",
    "шт",
    "компл",
    "ед",
    "т",
    "кг",
    "л",
];

pub fn parse(sheet: &str, row: &[Cell], columns: Option<&Columns>) -> Option<ParsedLine> {
    let raw_text = raw_text(row);
    if raw_text.is_empty() {
        return None;
    }
    let mut parsed = ParsedLine {
        sheet: sheet.to_owned(),
        raw_text,
        ..ParsedLine::default()
    };
    match columns {
        Some(columns) => fill_by_columns(&mut parsed, row, columns),
        None => fill_by_last_number(&mut parsed, row),
    }
    if parsed.unit.is_none() {
        parsed.unit = unit_of(row);
    }
    // название без единого числа — это заголовок раздела («СТЕНЫ»), а не
    // работа: такую строку оставляем сырой, чтобы не выдавать её за позицию
    if !parsed.is_recognized() {
        parsed.title = None;
        parsed.unit = None;
    }
    Some(parsed)
}

/// Строка без распознавания: над шапкой таблицы лежат название объекта и
/// реквизиты — их показываем как есть, а поля работ к ним не примеряем
pub fn raw(sheet: &str, row: &[Cell]) -> Option<ParsedLine> {
    let raw_text = raw_text(row);
    (!raw_text.is_empty()).then(|| ParsedLine {
        sheet: sheet.to_owned(),
        raw_text,
        ..ParsedLine::default()
    })
}

fn raw_text(row: &[Cell]) -> String {
    let joined = row
        .iter()
        .map(|cell| cell.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    joined.chars().take(MAX_RAW_LEN).collect()
}

fn fill_by_columns(parsed: &mut ParsedLine, row: &[Cell], columns: &Columns) {
    parsed.title = columns.title.and_then(|index| title_at(row, index));
    parsed.unit = columns
        .unit
        .and_then(|index| row.get(index))
        .map(|cell| cell.text.trim().to_owned())
        .filter(|unit| !unit.is_empty() && unit.chars().count() <= MAX_UNIT_LEN);
    parsed.quantity = columns.quantity.and_then(|index| number_at(row, index));
    parsed.price = columns.price.and_then(|index| number_at(row, index));
    parsed.total = columns.total.and_then(|index| number_at(row, index));
    // бывает, что колонка названия пуста, а работа записана в соседнюю —
    // тогда берём первый подходящий текст, иначе потеряли бы строку целиком
    if parsed.title.is_none() {
        parsed.title = first_title(row);
    }
}

/// Без шапки берём только последнее число строки: в сметах это итог, и
/// ошибиться в нём труднее всего.
fn fill_by_last_number(parsed: &mut ParsedLine, row: &[Cell]) {
    parsed.title = first_title(row);
    parsed.total = row.iter().filter_map(|cell| cell.number).next_back();
}

fn title_at(row: &[Cell], index: usize) -> Option<String> {
    let cell = row.get(index)?;
    let text = cell.text.trim();
    // «кв.м.» длиннее трёх букв, но названием работы быть не может
    (cell.number.is_none() && text.chars().count() >= MIN_TITLE_LEN && !is_unit(text))
        .then(|| text.to_owned())
}

fn first_title(row: &[Cell]) -> Option<String> {
    (0..row.len()).find_map(|index| title_at(row, index))
}

fn number_at(row: &[Cell], index: usize) -> Option<f64> {
    row.get(index).and_then(|cell| cell.number)
}

fn unit_of(row: &[Cell]) -> Option<String> {
    row.iter().find_map(|cell| {
        let text = cell.text.trim();
        is_unit(text).then(|| text.to_owned())
    })
}

fn is_unit(text: &str) -> bool {
    let normalized = text.to_lowercase().replace(' ', "");
    UNITS
        .iter()
        .any(|unit| normalized == *unit || normalized == format!("{unit}."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[(&str, Option<f64>)]) -> Vec<Cell> {
        cells
            .iter()
            .map(|(text, number)| Cell {
                text: (*text).to_owned(),
                number: *number,
            })
            .collect()
    }

    fn columns() -> Columns {
        Columns {
            header_row: 0,
            title: Some(0),
            unit: Some(1),
            quantity: Some(2),
            price: Some(3),
            total: Some(4),
        }
    }

    #[test]
    fn work_line_is_parsed_by_columns() {
        let cells = row(&[
            ("Штукатурка стен", None),
            ("кв.м.", None),
            ("12", Some(12.0)),
            ("740", Some(740.0)),
            ("8880", Some(8880.0)),
        ]);
        let parsed = parse("Смета", &cells, Some(&columns())).expect("строка разобрана");
        assert_eq!(parsed.title.as_deref(), Some("Штукатурка стен"));
        assert_eq!(parsed.unit.as_deref(), Some("кв.м."));
        assert_eq!(parsed.quantity, Some(12.0));
        assert_eq!(parsed.price, Some(740.0));
        assert_eq!(parsed.total, Some(8880.0));
        assert!(parsed.is_recognized());
    }

    #[test]
    fn section_header_stays_raw() {
        let cells = row(&[("СТЕНЫ", None)]);
        let parsed = parse("Смета", &cells, Some(&columns())).expect("строка есть");
        assert_eq!(parsed.title, None, "заголовок раздела — не позиция сметы");
        assert_eq!(parsed.raw_text, "СТЕНЫ");
    }

    #[test]
    fn price_written_words_leaves_the_line_partly_raw() {
        // «от 600» — не число: цену не выдумываем, но строку сохраняем
        let cells = row(&[
            ("Натяжной потолок", None),
            ("кв.м.", None),
            ("5", Some(5.0)),
            ("от 600", None),
            ("3000", Some(3000.0)),
        ]);
        let parsed = parse("Смета", &cells, Some(&columns())).expect("строка разобрана");
        assert_eq!(parsed.price, None);
        assert_eq!(parsed.quantity, Some(5.0));
        assert!(parsed.raw_text.contains("от 600"));
    }

    #[test]
    fn without_header_only_the_last_number_is_trusted() {
        let cells = row(&[
            ("Демонтаж плитки", None),
            ("12", Some(12.0)),
            ("540", Some(540.0)),
            ("6480", Some(6480.0)),
        ]);
        let parsed = parse("Лист1", &cells, None).expect("строка разобрана");
        assert_eq!(parsed.title.as_deref(), Some("Демонтаж плитки"));
        assert_eq!(parsed.total, Some(6480.0));
        assert_eq!(parsed.price, None, "цену без шапки не угадываем");
    }

    #[test]
    fn empty_row_is_skipped() {
        assert!(parse("Смета", &row(&[("", None), ("  ", None)]), None).is_none());
    }

    #[test]
    fn unit_is_not_mistaken_for_a_title() {
        // колонка названия пуста, работа записана дальше — «кв.м.» из
        // соседней ячейки названием не становится
        let cells = row(&[
            ("", None),
            ("кв.м.", None),
            ("Стяжка пола", None),
            ("8", Some(8.0)),
        ]);
        let parsed = parse("Смета", &cells, None).expect("строка разобрана");
        assert_eq!(parsed.title.as_deref(), Some("Стяжка пола"));
        assert_eq!(parsed.unit.as_deref(), Some("кв.м."));
    }
}
