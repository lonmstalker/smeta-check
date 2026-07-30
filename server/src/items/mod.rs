//! Примерная бизнес-сущность. При старте настоящего проекта переименовать
//! в первую реальную сущность — структура домена уже показывает, как надо.
//!
//! Здесь же образец row-level authorization: у записи есть владелец, и КАЖДЫЙ
//! запрос ограничен им прямо в SQL. Проверять права отдельным `if` после
//! выборки нельзя: однажды такой `if` забудут, а условие в SQL не забудешь.

pub mod http;

use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, sqlx::FromRow, ToSchema)]
pub struct Item {
    pub id: i64,
    pub title: String,
}

/// Страница списка своих записей, новые сверху: записи с id меньше `cursor`
/// (id последней записи прошлой страницы), первые `limit` штук
pub async fn list(
    pool: &PgPool,
    owner: Uuid,
    cursor: Option<i64>,
    limit: i64,
) -> sqlx::Result<Vec<Item>> {
    sqlx::query_as(
        "SELECT id, title FROM items
         WHERE owner_user_id = $1 AND id < $2
         ORDER BY id DESC LIMIT $3",
    )
    .bind(owner)
    .bind(cursor.unwrap_or(i64::MAX))
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn create(pool: &PgPool, owner: Uuid, title: &str) -> sqlx::Result<Item> {
    sqlx::query_as("INSERT INTO items (owner_user_id, title) VALUES ($1, $2) RETURNING id, title")
        .bind(owner)
        .bind(title)
        .fetch_one(pool)
        .await
}

/// true, если запись существовала и принадлежала владельцу. Чужая и
/// несуществующая запись неотличимы снаружи — обе дают 404, поэтому по ответу
/// нельзя узнать, что запись с таким id вообще есть.
pub async fn delete(pool: &PgPool, owner: Uuid, id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM items WHERE id = $1 AND owner_user_id = $2")
        .bind(id)
        .bind(owner)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Удаление без оглядки на владельца — только для модерации (роль admin)
pub async fn delete_any(pool: &PgPool, id: i64) -> sqlx::Result<bool> {
    let result = sqlx::query("DELETE FROM items WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}
