//! Восстановление пароля: запрос ссылки и установка нового пароля.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::auth;
use crate::core::config::Settings;
use crate::core::error::ApiError;

#[derive(Deserialize, ToSchema)]
pub struct ForgotRequest {
    pub email: String,
}

#[utoipa::path(post, path = "/api/auth/forgot", tag = "auth",
    request_body = ForgotRequest, responses((status = 202)))]
pub(crate) async fn forgot(
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    Json(req): Json<ForgotRequest>,
) -> Result<StatusCode, ApiError> {
    auth::forgot_password(&pool, &settings.public_url, &req.email).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize, ToSchema)]
pub struct ResetRequest {
    pub token: String,
    #[schema(min_length = 8)]
    pub password: String,
}

#[utoipa::path(post, path = "/api/auth/reset", tag = "auth",
    request_body = ResetRequest,
    responses((status = 204), (status = 422, body = crate::core::error::ErrorBody)))]
pub(crate) async fn reset(
    State(pool): State<PgPool>,
    Json(req): Json<ResetRequest>,
) -> Result<StatusCode, ApiError> {
    auth::reset_password(&pool, &req.token, &req.password).await?;
    Ok(StatusCode::NO_CONTENT)
}
