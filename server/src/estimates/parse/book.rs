//! Чтение книги Excel: открыть файл, обойти листы, отдать строки текстом.
//! Формат (xlsx или xls) calamine определяет сам по содержимому — расширению
//! из имени файла верить нельзя.

use std::io::{Cursor, Read, Seek};

use calamine::{Data, Reader, Sheets, Xlsx};

use super::{MAX_COLUMNS, MAX_ROWS_PER_SHEET, MAX_SHEETS, ParseError};

/// Потолки для «холостого» прохода по ячейкам xlsx (см. `check_bounds`).
/// Колонок берём с запасом: в живых сметах данные заезжают и в тридцатую.
const MAX_BOUND_COLUMNS: u32 = 1_000;
const MAX_CELLS: usize = 500_000;

/// Лист книги: имя и строки, каждая — набор ячеек
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<Cell>>,
}

/// Ячейка: текст, как его видел человек, и число, если оно там есть
#[derive(Debug, Clone, Default)]
pub struct Cell {
    pub text: String,
    pub number: Option<f64>,
}

pub fn read(bytes: Vec<u8>) -> Result<Vec<Sheet>, ParseError> {
    let cursor = Cursor::new(bytes);
    let book = calamine::open_workbook_auto_from_rs(cursor).map_err(|err| {
        tracing::info!(error = %err, "файл сметы не открылся как книга Excel");
        ParseError::Unreadable
    })?;
    match book {
        Sheets::Xlsx(mut book) => {
            check_bounds(&mut book)?;
            sheets_of(&mut book)
        }
        // .xls не сжат, поэтому «бомбой» быть не может: его размер на диске
        // и есть его размер в памяти
        Sheets::Xls(mut book) => sheets_of(&mut book),
        // расширение обещало xlsx или xls; ods и xlsb внутри — не наш случай
        _ => Err(ParseError::Unreadable),
    }
}

/// Холостой проход по ячейкам xlsx до чтения листа целиком.
///
/// xlsx — это zip, и файл на сто килобайт разворачивается в лист с ячейкой
/// в миллионном ряду. calamine хранит лист плотной таблицей по границам
/// реальных ячеек, поэтому такая ячейка означает попытку выделить сотни
/// гигабайт — то есть смерть процесса. Здесь мы смотрим на координаты
/// заранее и отказываемся раньше, чем что-то выделено.
fn check_bounds(book: &mut Xlsx<Cursor<Vec<u8>>>) -> Result<(), ParseError> {
    for name in book.sheet_names() {
        let Ok(mut reader) = book.worksheet_cells_reader(&name) else {
            continue;
        };
        let mut cells = 0usize;
        while let Ok(Some(cell)) = reader.next_cell() {
            let (row, column) = cell.get_position();
            if row as usize >= MAX_ROWS_PER_SHEET || column >= MAX_BOUND_COLUMNS {
                return Err(ParseError::TooBig);
            }
            cells += 1;
            if cells > MAX_CELLS {
                return Err(ParseError::TooBig);
            }
        }
    }
    Ok(())
}

fn sheets_of<RS, R>(book: &mut R) -> Result<Vec<Sheet>, ParseError>
where
    RS: Read + Seek,
    R: Reader<RS>,
{
    let names = book.sheet_names();
    if names.len() > MAX_SHEETS {
        return Err(ParseError::TooBig);
    }
    let mut sheets = Vec::new();
    for name in names {
        let Ok(range) = book.worksheet_range(&name) else {
            // один нечитаемый лист не повод терять остальные
            tracing::info!(sheet = name, "лист не прочитался, пропускаем");
            continue;
        };
        let (rows, columns) = range.get_size();
        if rows > MAX_ROWS_PER_SHEET {
            return Err(ParseError::TooBig);
        }
        let width = columns.min(MAX_COLUMNS);
        let rows = range
            .rows()
            .map(|row| row.iter().take(width).map(cell).collect())
            .collect();
        sheets.push(Sheet { name, rows });
    }
    Ok(sheets)
}

fn cell(value: &Data) -> Cell {
    Cell {
        text: text_of(value),
        number: number_of(value),
    }
}

fn text_of(value: &Data) -> String {
    match value {
        Data::String(s) => s.trim().to_owned(),
        Data::Float(f) => format_float(*f),
        Data::Int(i) => i.to_string(),
        Data::Bool(b) => b.to_string(),
        // даты в смете встречаются в шапке («от 10.01.2021»), нам их хватает
        // строкой; ошибки формул и пустые ячейки текста не дают
        Data::DateTime(d) => d.to_string(),
        _ => String::new(),
    }
}

/// Число из ячейки. Строку «1 234,56» считаем числом, а «от 600» — нет:
/// такая цена не число, и строка честно останется нераспознанной.
fn number_of(value: &Data) -> Option<f64> {
    match value {
        Data::Float(f) => Some(*f),
        Data::Int(i) => Some(*i as f64),
        Data::String(s) => parse_number(s),
        _ => None,
    }
}

pub fn parse_number(raw: &str) -> Option<f64> {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\'')
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    cleaned.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Excel хранит числа как f64, и 3779.9999999999995 в файле — это 3780
/// в глазах человека. Округляем до сотых и убираем хвост нулей.
fn format_float(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let text = format!("{rounded}");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_read_from_text_too() {
        assert_eq!(parse_number("1 234,56"), Some(1234.56));
        assert_eq!(parse_number("12"), Some(12.0));
        assert_eq!(parse_number("от 600"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("кв.м."), None);
    }

    #[test]
    fn float_text_has_no_excel_tail() {
        assert_eq!(format_float(3779.9999999999995), "3780");
        assert_eq!(format_float(12.5), "12.5");
    }
}
