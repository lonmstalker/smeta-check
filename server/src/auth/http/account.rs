//! Настройки аккаунта: смена пароля и смена адреса почты.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use super::REFRESH_COOKIE;
use crate::auth::account;
use crate::core::config::Settings;
use crate::core::error::ApiError;
use crate::users::http::CurrentUser;

#[derive(Deserialize, ToSchema)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    #[schema(min_length = 8)]
    pub new_password: String,
}

#[utoipa::path(post, path = "/api/auth/password", tag = "account",
    request_body = ChangePasswordRequest,
    responses((status = 204, description = "пароль изменён, все сессии закрыты"),
              (status = 422, body = crate::core::error::ErrorBody)),
    security(("bearer" = [])))]
pub(crate) async fn change_password(
    user: CurrentUser,
    State(pool): State<PgPool>,
    jar: CookieJar,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Response, ApiError> {
    account::change_password(&pool, user.id, &req.current_password, &req.new_password).await?;
    // сессии погашены, включая текущую — забираем и cookie, чтобы браузер
    // не ходил с заведомо мёртвым токеном
    let jar = jar.remove(REFRESH_COOKIE);
    Ok((StatusCode::NO_CONTENT, jar).into_response())
}

#[derive(Deserialize, ToSchema)]
pub struct ChangeEmailRequest {
    #[schema(format = "email")]
    pub new_email: String,
    pub current_password: String,
}

#[utoipa::path(post, path = "/api/auth/email", tag = "account",
    request_body = ChangeEmailRequest,
    responses((status = 202, description = "письмо с подтверждением отправлено"),
              (status = 409, body = crate::core::error::ErrorBody),
              (status = 422, body = crate::core::error::ErrorBody)),
    security(("bearer" = [])))]
pub(crate) async fn request_email_change(
    user: CurrentUser,
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    Json(req): Json<ChangeEmailRequest>,
) -> Result<StatusCode, ApiError> {
    account::request_email_change(
        &pool,
        &settings.public_url,
        user.id,
        &req.new_email,
        &req.current_password,
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize, ToSchema)]
pub struct ConfirmEmailRequest {
    pub token: String,
}

#[utoipa::path(post, path = "/api/auth/email/confirm", tag = "account",
    request_body = ConfirmEmailRequest,
    responses((status = 204), (status = 422, body = crate::core::error::ErrorBody)))]
pub(crate) async fn confirm_email_change(
    State(pool): State<PgPool>,
    Json(req): Json<ConfirmEmailRequest>,
) -> Result<StatusCode, ApiError> {
    account::confirm_email_change(&pool, &req.token).await?;
    Ok(StatusCode::NO_CONTENT)
}
