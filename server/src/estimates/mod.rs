//! Сметы пользователя: карточка загруженного файла и правила приёма.
//!
//! Сметы личные, поэтому каждая выборка ограничена владельцем прямо в SQL:
//! чужую смету нельзя достать даже по ошибке в новом запросе, а чужая и
//! несуществующая снаружи неотличимы (обе — 404).

pub mod http;
pub mod parse;
pub mod worker;

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::core::i18n;
use crate::core::time::rfc3339;
use crate::estimates::parse::ParsedLine;

/// 10 МиБ. Смета в Excel столько не весит даже со сканами; больше — это уже
/// не смета, и принимать её значит бесплатно отдавать диск.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

/// Excel разбирает алгоритмика, фотографию — нейросеть. PDF пока не
/// принимаем: разбирать его нечем, и принятый файл завис бы в «загружено».
pub const ALLOWED_EXTS: [&str; 6] = ["xlsx", "xls", "jpg", "jpeg", "png", "webp"];

/// Фото ли это. Дальше по этому признаку расходятся и правила приёма
/// (нейросеть включена, почта подтверждена), и способ разбора.
pub fn is_photo(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "png" | "webp")
}

/// Похож ли файл на картинку заявленного вида — по первым байтам. Проверяем
/// на загрузке, а не в разборе: переименованный в .jpg архив не должен
/// стоить трёх вызовов нейросети.
pub fn photo_matches_ext(ext: &str, bytes: &[u8]) -> bool {
    match ext {
        "jpg" | "jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP".as_slice()),
        // не фото: Excel честно падает в разборе с читаемой причиной
        _ => true,
    }
}

/// Потолок смет на аккаунт: без него диск заливается 10-мегабайтными файлами
/// бесплатно и бесконечно.
pub const MAX_PER_USER: i64 = 20;

/// Имя файла показываем в списке; всё длиннее — способ сломать вёрстку
const MAX_FILE_NAME_LEN: usize = 200;

/// Смета глазами пользователя
#[derive(Serialize, ToSchema)]
pub struct Estimate {
    pub id: Uuid,
    /// имя файла, как его назвал сам пользователь
    pub file_name: String,
    pub size_bytes: i64,
    /// uploaded — принята, parsing — разбирается, parsed — разобрана,
    /// failed — разобрать не смогли
    pub status: String,
    /// почему не разобрали — на языке запроса; есть только у failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// смету прислали фотографией — её строки распознала нейросеть, и
    /// интерфейс обязан честно об этом предупредить
    pub from_photo: bool,
    /// RFC 3339 в UTC; в местное время переводит браузер
    pub created_at: String,
}

/// Строка сметы: сырой текст плюс то, что удалось распознать. Пустые поля —
/// честный ответ «это место мы не поняли», а не потеря данных.
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct EstimateLine {
    /// порядок строки в файле
    pub position: i32,
    pub sheet: String,
    /// строка целиком, как её видел человек в Excel
    pub raw_text: String,
    /// название работы; null — строку не разобрали
    pub title: Option<String>,
    pub unit: Option<String>,
    pub quantity: Option<f64>,
    pub price: Option<f64>,
    pub total: Option<f64>,
}

#[derive(sqlx::FromRow)]
struct EstimateRow {
    id: Uuid,
    file_name: String,
    file_ext: String,
    size_bytes: i64,
    status: String,
    error_key: Option<String>,
    created_at: OffsetDateTime,
}

impl EstimateRow {
    fn into_estimate(self) -> Estimate {
        Estimate {
            id: self.id,
            file_name: self.file_name,
            from_photo: is_photo(&self.file_ext),
            size_bytes: self.size_bytes,
            status: self.status,
            // в базе лежит ключ локализации, текст подставляем на языке
            // запроса — так одна и та же смета объясняется по-русски и
            // по-английски без второй записи в базе
            error: self
                .error_key
                .map(|key| i18n::translate(i18n::current_lang(), &key, &[])),
            created_at: rfc3339(self.created_at),
        }
    }
}

/// Расширение из имени файла, если такое мы принимаем. Возвращаем не кусок
/// пользовательской строки, а значение из списка — из него потом собирается
/// имя файла на диске.
pub fn extension_of(file_name: &str) -> Option<&'static str> {
    let ext = file_name.rsplit_once('.')?.1.to_ascii_lowercase();
    ALLOWED_EXTS.into_iter().find(|allowed| *allowed == ext)
}

/// Имя файла на диске: от id записи, ничего пользовательского внутри
pub fn stored_name(id: Uuid, ext: &str) -> String {
    format!("{id}.{ext}")
}

/// Обрезать имя до разумного, убрав пробелы по краям
pub fn clean_file_name(file_name: &str) -> String {
    file_name.trim().chars().take(MAX_FILE_NAME_LEN).collect()
}

/// Создать карточку сметы, если потолок владельца ещё не выбран. None —
/// лимит: проверка и вставка идут под замком по владельцу, поэтому два
/// одновременных запроса не протащат лишнюю смету.
pub async fn create(
    pool: &PgPool,
    id: Uuid,
    owner: Uuid,
    file_name: &str,
    ext: &str,
    size_bytes: i64,
) -> sqlx::Result<Option<Estimate>> {
    let mut tx = pool.begin().await?;
    // замок транзакционный: отпускается сам на commit или rollback
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(owner)
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM estimates WHERE owner_user_id = $1")
        .bind(owner)
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_PER_USER {
        return Ok(None);
    }
    let row: EstimateRow = sqlx::query_as(
        "INSERT INTO estimates (id, owner_user_id, file_name, file_ext, size_bytes)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, file_name, file_ext, size_bytes, status, error_key, created_at",
    )
    .bind(id)
    .bind(owner)
    .bind(file_name)
    .bind(ext)
    .bind(size_bytes)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(row.into_estimate()))
}

/// Свои сметы, новые сверху. Больше `MAX_PER_USER` их не бывает, поэтому
/// страниц здесь нет.
pub async fn list(pool: &PgPool, owner: Uuid) -> sqlx::Result<Vec<Estimate>> {
    let rows: Vec<EstimateRow> = sqlx::query_as(
        "SELECT id, file_name, file_ext, size_bytes, status, error_key, created_at FROM estimates
         WHERE owner_user_id = $1 ORDER BY created_at DESC",
    )
    .bind(owner)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(EstimateRow::into_estimate).collect())
}

/// Своя смета по id. None — нет такой или она чужая: снаружи это одно и то же.
pub async fn get(pool: &PgPool, owner: Uuid, id: Uuid) -> sqlx::Result<Option<Estimate>> {
    let row: Option<EstimateRow> = sqlx::query_as(
        "SELECT id, file_name, file_ext, size_bytes, status, error_key, created_at FROM estimates
         WHERE id = $1 AND owner_user_id = $2",
    )
    .bind(id)
    .bind(owner)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(EstimateRow::into_estimate))
}

pub async fn count_of(pool: &PgPool, owner: Uuid) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT count(*) FROM estimates WHERE owner_user_id = $1")
        .bind(owner)
        .fetch_one(pool)
        .await
}

/// Строки сметы по порядку, как в файле
pub async fn lines_of(pool: &PgPool, estimate_id: Uuid) -> sqlx::Result<Vec<EstimateLine>> {
    sqlx::query_as(
        "SELECT position, sheet, raw_text, title, unit, quantity, price, total
         FROM estimate_lines WHERE estimate_id = $1 ORDER BY position",
    )
    .bind(estimate_id)
    .fetch_all(pool)
    .await
}

/// Заменить строки сметы разобранными. Старые удаляем: разбор мог повториться
/// после перезапуска, и удвоенные строки — это уже неверная смета.
pub async fn replace_lines(
    pool: &PgPool,
    estimate_id: Uuid,
    lines: &[ParsedLine],
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM estimate_lines WHERE estimate_id = $1")
        .bind(estimate_id)
        .execute(&mut *tx)
        .await?;
    // одним запросом через UNNEST: пять тысяч отдельных INSERT'ов заняли бы
    // секунды и держали транзакцию открытой всё это время
    sqlx::query(
        "INSERT INTO estimate_lines
             (estimate_id, position, sheet, raw_text, title, unit, quantity, price, total)
         SELECT $1, * FROM UNNEST(
             $2::int[], $3::text[], $4::text[], $5::text[],
             $6::text[], $7::float8[], $8::float8[], $9::float8[])",
    )
    .bind(estimate_id)
    .bind((0..lines.len() as i32).collect::<Vec<_>>())
    .bind(lines.iter().map(|l| l.sheet.clone()).collect::<Vec<_>>())
    .bind(lines.iter().map(|l| l.raw_text.clone()).collect::<Vec<_>>())
    .bind(lines.iter().map(|l| l.title.clone()).collect::<Vec<_>>())
    .bind(lines.iter().map(|l| l.unit.clone()).collect::<Vec<_>>())
    .bind(lines.iter().map(|l| l.quantity).collect::<Vec<_>>())
    .bind(lines.iter().map(|l| l.price).collect::<Vec<_>>())
    .bind(lines.iter().map(|l| l.total).collect::<Vec<_>>())
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_taken_from_the_allowed_list() {
        assert_eq!(extension_of("смета.XLSX"), Some("xlsx"));
        assert_eq!(extension_of("smeta.xls"), Some("xls"));
        assert_eq!(extension_of("smeta.pdf"), None);
        assert_eq!(extension_of("smeta"), None);
        // расширение из имени наружу не проходит: путь подменить нечем
        assert_eq!(extension_of("smeta.xlsx/../../etc/passwd"), None);
    }

    #[test]
    fn long_file_name_is_cut() {
        let name = clean_file_name(&format!("  {}  ", "я".repeat(500)));
        assert_eq!(name.chars().count(), MAX_FILE_NAME_LEN);
    }
}
