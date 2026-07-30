//! Список активных сессий и их отзыв. Маршруты живут под /api/auth, потому что
//! refresh-cookie отправляется браузером только туда — без неё нельзя понять,
//! какая из сессий текущая.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum_extra::extract::cookie::CookieJar;
use sqlx::PgPool;
use uuid::Uuid;

use super::REFRESH_COOKIE;
use crate::auth::sessions::{self, SessionInfo};
use crate::core::error::ApiError;
use crate::users::http::CurrentUser;

#[utoipa::path(get, path = "/api/auth/sessions", tag = "account",
    responses((status = 200, body = Vec<SessionInfo>)), security(("bearer" = [])))]
pub(crate) async fn list(
    user: CurrentUser,
    State(pool): State<PgPool>,
    jar: CookieJar,
) -> Result<Json<Vec<SessionInfo>>, ApiError> {
    let current = jar.get(REFRESH_COOKIE).map(|c| c.value().to_owned());
    Ok(Json(
        sessions::list(&pool, user.id, current.as_deref()).await?,
    ))
}

#[utoipa::path(delete, path = "/api/auth/sessions/{id}", tag = "account",
    responses((status = 204), (status = 404)), security(("bearer" = [])))]
pub(crate) async fn revoke(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    // чужая и несуществующая сессия отвечают одинаково
    if !sessions::revoke(&pool, user.id, id).await? {
        return Err(ApiError::not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(delete, path = "/api/auth/sessions", tag = "account",
    responses((status = 204, description = "все остальные сессии закрыты"), (status = 401)),
    security(("bearer" = [])))]
pub(crate) async fn revoke_others(
    user: CurrentUser,
    State(pool): State<PgPool>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let current = jar
        .get(REFRESH_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))?;
    sessions::revoke_others(&pool, user.id, &current).await?;
    Ok(StatusCode::NO_CONTENT)
}
