use std::sync::Arc;

use axum::extract::{FromRef, FromRequestParts, State};
use axum::http::request::Parts;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;
use crate::auth::jwt;
use crate::core::config::Settings;
use crate::core::error::ApiError;
use crate::core::i18n;
use crate::users::{self, Role, User};

/// Длиннее — это уже не имя, а способ сломать вёрстку у соседей
const MAX_DISPLAY_NAME_LEN: usize = 100;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/users/me", get(me).patch(update_me))
}

/// Проверенный access-токен запроса. Добавь параметром — маршрут станет
/// доступен только вошедшим: `async fn handler(user: CurrentUser, ...)`
pub struct CurrentUser {
    pub id: Uuid,
    pub role: Role,
}

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    Arc<Settings>: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let settings = Arc::<Settings>::from_ref(state);
        let token = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))?;
        let claims = jwt::verify_access(&settings.jwt_secret, token)
            .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))?;
        Ok(CurrentUser {
            id: claims.sub,
            role: Role::parse(&claims.role),
        })
    }
}

/// То же, но пускает только администратора
pub struct AdminUser(pub CurrentUser);

impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
    Arc<Settings>: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if user.role != Role::Admin {
            return Err(ApiError::forbidden());
        }
        Ok(AdminUser(user))
    }
}

#[utoipa::path(get, path = "/api/users/me", tag = "users",
    responses((status = 200, body = User)), security(("bearer" = [])))]
pub(crate) async fn me(
    user: CurrentUser,
    State(pool): State<PgPool>,
) -> Result<Json<User>, ApiError> {
    let record = users::find_by_id(&pool, user.id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))?;
    Ok(Json(record.to_user()))
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMe {
    /// как обращаться к пользователю; пропущено — не меняем
    #[schema(max_length = 100)]
    pub display_name: Option<String>,
    /// язык интерфейса и писем: "ru" или "en"
    pub locale: Option<String>,
}

#[utoipa::path(patch, path = "/api/users/me", tag = "account",
    request_body = UpdateMe,
    responses((status = 200, body = User),
              (status = 422, body = crate::core::error::ErrorBody)),
    security(("bearer" = [])))]
pub(crate) async fn update_me(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Json(req): Json<UpdateMe>,
) -> Result<Json<User>, ApiError> {
    let display_name = req.display_name.map(|name| name.trim().to_owned());
    if let Some(name) = &display_name
        && name.chars().count() > MAX_DISPLAY_NAME_LEN
    {
        return Err(ApiError::validation("error-name-long")
            .arg("max", MAX_DISPLAY_NAME_LEN)
            .field("display_name"));
    }
    if let Some(locale) = &req.locale
        && !i18n::ALL_LANGS.iter().any(|(_, code)| code == locale)
    {
        return Err(ApiError::validation("error-unknown-locale").field("locale"));
    }
    users::update_profile(
        &pool,
        user.id,
        display_name.as_deref(),
        req.locale.as_deref(),
    )
    .await?;
    let record = users::find_by_id(&pool, user.id)
        .await?
        .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))?;
    Ok(Json(record.to_user()))
}
