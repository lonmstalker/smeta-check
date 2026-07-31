//! Поиск шапки таблицы и колонок.
//!
//! В настоящих сметах шапка почти никогда не в первой строке: сверху бывают
//! название объекта, реквизиты и пустые строки. Ищем строку, где знакомые
//! слова («наименование», «кол-во», «цена», «сумма») сошлись хотя бы в двух
//! РАЗНЫХ колонках: два слова в одной ячейке — это фраза («…с объемами
//! работ…»), а не шапка.

use super::book::Cell;

/// Насколько глубоко ищем шапку: ниже начинаются сами работы
const SEARCH_ROWS: usize = 40;

/// Знакомые слова должны найтись хотя бы в стольких разных колонках
const MIN_MATCHED_COLUMNS: usize = 2;

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
    /// Сколько РАЗНЫХ колонок опознано: слоты, сошедшиеся в одну колонку,
    /// считаются один раз, иначе шапкой станет обычное предложение
    fn matched_columns(&self) -> usize {
        let mut indices: Vec<usize> =
            [self.title, self.unit, self.quantity, self.price, self.total]
                .into_iter()
                .flatten()
                .collect();
        indices.sort_unstable();
        indices.dedup();
        indices.len()
    }
}

/// Слова шапки. Порядок важен: «стоимость единицы» — это цена, поэтому цену
/// проверяем раньше суммы.
const TITLE_WORDS: [&str; 6] = [
    "наименование",
    "вид работ",
    "виды работ",
    "позиция",
    "товар",
    "работы",
];
const UNIT_WORDS: [&str; 3] = ["ед.", "ед ", "единиц"];
/// «кол» покрывает и «Кол-во», и «Количество», и просто «Кол»
const QUANTITY_WORDS: [&str; 3] = ["кол", "объем", "объём"];
const PRICE_WORDS: [&str; 4] = ["цена", "расценка", "стоимость ед", "цена ед"];
/// Однозначные слова суммы строки. Голой «стоимости» здесь нет: рядом с
/// «итого» она означает цену за единицу — см. развязку в `columns_of`
const TOTAL_WORDS: [&str; 3] = ["сумма", "всего", "итого"];
const COST_WORD: &str = "стоимость";

pub fn find(rows: &[Vec<Cell>]) -> Option<Columns> {
    rows.iter()
        .take(SEARCH_ROWS)
        .enumerate()
        .map(|(index, row)| columns_of(index, row))
        .find(|columns| columns.matched_columns() >= MIN_MATCHED_COLUMNS)
}

fn columns_of(header_row: usize, row: &[Cell]) -> Columns {
    let mut columns = Columns {
        header_row,
        ..Columns::default()
    };
    // колонка с голой «стоимостью»: сумма это или цена — решаем после прохода
    let mut cost = None;
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
        if cost.is_none()
            && columns.price != Some(index)
            && columns.total != Some(index)
            && text.contains(COST_WORD)
        {
            cost = Some(index);
        }
    }
    // «Стоимость» без «итого» — сумма строки (обычная смета), а рядом с
    // «итого» — цена за единицу («… | Кол-во | Стоимость | Итого»)
    match (columns.total, cost) {
        (None, Some(index)) => columns.total = Some(index),
        (Some(_), Some(index)) if columns.price.is_none() => columns.price = Some(index),
        _ => {}
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

    #[test]
    fn sentence_with_two_words_in_one_cell_is_not_a_header() {
        // дисклеймер remplanner: «работы» и «объемами» в одной ячейке
        let rows = vec![
            row(&["Для работы используйте шаблоны с объемами работ по вашей квартире"]),
            row(&[
                "Демонтаж стен",
                "Расценка, руб",
                "Объем",
                "Ед. изм.",
                "Стоимость, руб",
            ]),
        ];
        let columns = find(&rows).expect("настоящая шапка строкой ниже");
        assert_eq!(columns.header_row, 1);
        assert_eq!(columns.price, Some(1));
        assert_eq!(columns.quantity, Some(2));
        assert_eq!(
            columns.total,
            Some(4),
            "одинокая «стоимость» — сумма строки"
        );
    }

    #[test]
    fn cost_next_to_itogo_is_a_price_not_a_total() {
        let rows = vec![row(&[
            "№ п/п",
            "Наименование работ",
            "Ед.изм.",
            "Кол-во",
            "Стоимость",
            "Итого",
        ])];
        let columns = find(&rows).expect("шапка");
        assert_eq!(
            columns.price,
            Some(4),
            "«Стоимость» рядом с «Итого» — цена за единицу"
        );
        assert_eq!(columns.total, Some(5));
    }

    #[test]
    fn bare_kol_column_is_quantity() {
        let rows = vec![row(&[
            "№",
            "Наименование затрат",
            "Кол",
            "Цена,р",
            "Стоимость, руб",
        ])];
        let columns = find(&rows).expect("шапка");
        assert_eq!(columns.quantity, Some(2));
        assert_eq!(columns.total, Some(4));
    }
}
