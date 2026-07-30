//! Записи личные: видит и меняет их только владелец (образец row-level
//! authorization). Админу дополнительно разрешено удалять чужие — это
//! единственная операция модерации в шаблоне.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::{IntoParams, ToSchema};

use crate::AppState;
use crate::core::error::ApiError;
use crate::items::{self, Item};
use crate::users::{Role, http::CurrentUser};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/items", get(list_items).post(create_item))
        .route("/api/items/{id}", axum::routing::delete(delete_item))
}

#[derive(Deserialize, IntoParams)]
pub struct ListParams {
    /// вернуть записи после этого id (next_cursor прошлой страницы)
    pub cursor: Option<i64>,
    /// размер страницы; по умолчанию 20, максимум 100
    pub limit: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct ItemsPage {
    pub items: Vec<Item>,
    /// есть продолжение — передайте это значение как cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<i64>,
}

#[utoipa::path(get, path = "/api/items", tag = "items",
    params(ListParams),
    responses((status = 200, body = ItemsPage), (status = 401)),
    security(("bearer" = [])))]
pub(crate) async fn list_items(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Query(params): Query<ListParams>,
) -> Result<Json<ItemsPage>, ApiError> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    // просим на одну запись больше: она и говорит, есть ли следующая страница
    let mut items = items::list(&pool, user.id, params.cursor, limit + 1).await?;
    let next_cursor = if items.len() as i64 > limit {
        items.truncate(limit as usize);
        items.last().map(|item| item.id)
    } else {
        None
    };
    Ok(Json(ItemsPage { items, next_cursor }))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateItem {
    #[schema(min_length = 1)]
    pub title: String,
}

#[utoipa::path(post, path = "/api/items", tag = "items",
    request_body = CreateItem,
    responses((status = 201, body = Item), (status = 401),
              (status = 422, body = crate::core::error::ErrorBody)),
    security(("bearer" = [])))]
pub(crate) async fn create_item(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Json(req): Json<CreateItem>,
) -> Result<(StatusCode, Json<Item>), ApiError> {
    let title = req.title.trim();
    if title.is_empty() {
        return Err(ApiError::validation("error-title-empty").field("title"));
    }
    let item = items::create(&pool, user.id, title).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[utoipa::path(delete, path = "/api/items/{id}", tag = "items",
    responses((status = 204), (status = 401), (status = 404)),
    security(("bearer" = [])))]
pub(crate) async fn delete_item(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    // админ удаляет любую запись (модерация), остальные — только свои;
    // чужая запись для обычного пользователя выглядит как несуществующая
    let deleted = match user.role {
        Role::Admin => items::delete_any(&pool, id).await?,
        Role::User => items::delete(&pool, user.id, id).await?,
    };
    if !deleted {
        return Err(ApiError::not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}
