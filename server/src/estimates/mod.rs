//! Сметы пользователя: карточка загруженного файла и правила приёма.
//!
//! Сметы личные, поэтому каждая выборка ограничена владельцем прямо в SQL:
//! чужую смету нельзя достать даже по ошибке в новом запросе, а чужая и
//! несуществующая снаружи неотличимы (обе — 404).

pub mod http;

use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::core::time::rfc3339;

/// 10 МиБ. Смета в Excel столько не весит даже со сканами; больше — это уже
/// не смета, и принимать её значит бесплатно отдавать диск.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

/// В этой волне принимаем только Excel: PDF и фото разбирать пока нечем, и
/// принятый файл навсегда завис бы в статусе «загружено».
pub const ALLOWED_EXTS: [&str; 2] = ["xlsx", "xls"];

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
    /// RFC 3339 в UTC; в местное время переводит браузер
    pub created_at: String,
}

#[derive(sqlx::FromRow)]
struct EstimateRow {
    id: Uuid,
    file_name: String,
    size_bytes: i64,
    status: String,
    created_at: OffsetDateTime,
}

impl EstimateRow {
    fn into_estimate(self) -> Estimate {
        Estimate {
            id: self.id,
            file_name: self.file_name,
            size_bytes: self.size_bytes,
            status: self.status,
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

pub async fn create(
    pool: &PgPool,
    id: Uuid,
    owner: Uuid,
    file_name: &str,
    ext: &str,
    size_bytes: i64,
) -> sqlx::Result<Estimate> {
    let row: EstimateRow = sqlx::query_as(
        "INSERT INTO estimates (id, owner_user_id, file_name, file_ext, size_bytes)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, file_name, size_bytes, status, created_at",
    )
    .bind(id)
    .bind(owner)
    .bind(file_name)
    .bind(ext)
    .bind(size_bytes)
    .fetch_one(pool)
    .await?;
    Ok(row.into_estimate())
}

/// Свои сметы, новые сверху. Больше `MAX_PER_USER` их не бывает, поэтому
/// страниц здесь нет.
pub async fn list(pool: &PgPool, owner: Uuid) -> sqlx::Result<Vec<Estimate>> {
    let rows: Vec<EstimateRow> = sqlx::query_as(
        "SELECT id, file_name, size_bytes, status, created_at FROM estimates
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
        "SELECT id, file_name, size_bytes, status, created_at FROM estimates
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
