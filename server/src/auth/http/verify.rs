//! Подтверждение адреса почты по одноразовой ссылке из письма.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::auth;
use crate::core::error::ApiError;

#[derive(Deserialize, ToSchema)]
pub struct VerifyEmailRequest {
    pub token: String,
}

#[utoipa::path(post, path = "/api/auth/verify-email", tag = "auth",
    request_body = VerifyEmailRequest, responses((status = 204), (status = 422)))]
pub(crate) async fn verify_email(
    State(pool): State<PgPool>,
    Json(req): Json<VerifyEmailRequest>,
) -> Result<StatusCode, ApiError> {
    auth::verify_email::verify_email(&pool, &req.token).await?;
    Ok(StatusCode::NO_CONTENT)
}
