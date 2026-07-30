//! Управление вторым фактором (TOTP) для вошедшего пользователя.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::auth;
use crate::core::error::ApiError;
use crate::users::http::CurrentUser;

#[derive(Serialize, ToSchema)]
pub struct TotpSetupResponse {
    /// показать пользователю для ручного ввода
    pub secret: String,
    /// ссылка otpauth:// для приложения-аутентификатора
    pub otpauth_url: String,
}

#[utoipa::path(post, path = "/api/auth/2fa/setup", tag = "auth",
    responses((status = 200, body = TotpSetupResponse)), security(("bearer" = [])))]
pub(crate) async fn setup(
    user: CurrentUser,
    State(pool): State<PgPool>,
) -> Result<Json<TotpSetupResponse>, ApiError> {
    let setup = auth::totp_setup(&pool, user.id).await?;
    Ok(Json(TotpSetupResponse {
        secret: setup.secret,
        otpauth_url: setup.otpauth_url,
    }))
}

#[derive(Deserialize, ToSchema)]
pub struct TotpEnableRequest {
    pub secret: String,
    pub code: String,
}

#[utoipa::path(post, path = "/api/auth/2fa/enable", tag = "auth",
    request_body = TotpEnableRequest,
    responses((status = 204), (status = 422)), security(("bearer" = [])))]
pub(crate) async fn enable(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Json(req): Json<TotpEnableRequest>,
) -> Result<StatusCode, ApiError> {
    auth::totp_enable(&pool, user.id, &req.secret, &req.code).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, ToSchema)]
pub struct TotpCodeRequest {
    pub code: String,
}

#[utoipa::path(post, path = "/api/auth/2fa/disable", tag = "auth",
    request_body = TotpCodeRequest,
    responses((status = 204), (status = 422)), security(("bearer" = [])))]
pub(crate) async fn disable(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Json(req): Json<TotpCodeRequest>,
) -> Result<StatusCode, ApiError> {
    auth::totp_disable(&pool, user.id, &req.code).await?;
    Ok(StatusCode::NO_CONTENT)
}
