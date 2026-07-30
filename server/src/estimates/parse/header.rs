//! Поиск шапки таблицы и колонок.
//!
//! В настоящих сметах шапка почти никогда не в первой строке: сверху бывают
//! название объекта, реквизиты и пустые строки. Ищем строку, где сошлись
//! хотя бы два знакомых слова («наименование», «кол-во», «цена», «сумма»),
//! и запоминаем номера колонок.

use super::book::Cell;

/// Насколько глубоко ищем шапку: ниже начинаются сами работы
const SEARCH_ROWS: usize = 40;

/// Колонок должно совпасть хотя бы столько, иначе это не шапка, а совпадение
const MIN_MATCHES: usize = 2;

#[derive(Debug, Default, PartialEq)]
pub struct Columns {
    pub header_row: usize,
    pub title: Option<usize>,
    pub unit: Option<usize>,
    pub quantity: Option<usize>,
    pub price: Option<usize>,
    pub total: Option<usize>,
}

impl Columns {
    fn matches(&self) -> usize {
        [self.title, self.unit, self.quantity, self.price, self.total]
            .into_iter()
            .flatten()
            .count()
    }
}

/// Слова шапки. Порядок важен: «стоимость единицы» — это цена, а просто
/// «стоимость» — сумма, поэтому цену проверяем раньше.
const TITLE_WORDS: [&str; 6] = [
    "наименование",
    "вид работ",
    "виды работ",
    "позиция",
    "товар",
    "работы",
];
const UNIT_WORDS: [&str; 3] = ["ед.", "ед ", "единиц"];
const QUANTITY_WORDS: [&str; 4] = ["кол-во", "количество", "объем", "объём"];
const PRICE_WORDS: [&str; 4] = ["цена", "расценка", "стоимость ед", "цена ед"];
const TOTAL_WORDS: [&str; 4] = ["сумма", "стоимость", "всего", "итого"];

pub fn find(rows: &[Vec<Cell>]) -> Option<Columns> {
    rows.iter()
        .take(SEARCH_ROWS)
        .enumerate()
        .map(|(index, row)| columns_of(index, row))
        .find(|columns| columns.matches() >= MIN_MATCHES)
}

fn columns_of(header_row: usize, row: &[Cell]) -> Columns {
    let mut columns = Columns {
        header_row,
        ..Columns::default()
    };
    for (index, cell) in row.iter().enumerate() {
        // «ед.изм.» пишут и с точками, и с пробелами — сравниваем по-простому
        let text = cell.text.to_lowercase();
        if text.is_empty() {
            continue;
        }
        let set = |slot: &mut Option<usize>, words: &[&str]| {
            if slot.is_none() && words.iter().any(|word| text.contains(word)) {
                *slot = Some(index);
            }
        };
        set(&mut columns.title, &TITLE_WORDS);
        set(&mut columns.unit, &UNIT_WORDS);
        set(&mut columns.quantity, &QUANTITY_WORDS);
        set(&mut columns.price, &PRICE_WORDS);
        // сумма не должна перехватить «стоимость единицы» — она уже цена
        if columns.price != Some(index) {
            set(&mut columns.total, &TOTAL_WORDS);
        }
    }
    columns
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cells: &[&str]) -> Vec<Cell> {
        cells
            .iter()
            .map(|text| Cell {
                text: (*text).to_owned(),
                number: None,
            })
            .collect()
    }

    #[test]
    fn header_is_found_below_the_title_block() {
        let rows = vec![
            row(&["Смета на ремонт двухкомнатной квартиры"]),
            row(&[""]),
            row(&["Наименование работ", "ед.изм.", "кол-во", "цена", "сумма"]),
            row(&["Штукатурка стен", "кв.м.", "12", "740", "8880"]),
        ];
        let columns = find(&rows).expect("шапка обязана найтись");
        assert_eq!(columns.header_row, 2);
        assert_eq!(columns.title, Some(0));
        assert_eq!(columns.unit, Some(1));
        assert_eq!(columns.quantity, Some(2));
        assert_eq!(columns.price, Some(3));
        assert_eq!(columns.total, Some(4));
    }

    #[test]
    fn table_without_header_is_not_invented() {
        let rows = vec![row(&["Штукатурка стен", "12", "740"])];
        assert_eq!(find(&rows), None);
    }
}
