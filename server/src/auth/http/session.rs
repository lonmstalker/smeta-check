//! Регистрация, вход (включая второй шаг 2FA), обновление и завершение сессии.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use super::{
    Credentials, LoginResponse, REFRESH_COOKIE, client_of, refresh_cookie, session_response,
};
use crate::auth::{self, LoginOutcome, jwt, sessions};
use crate::core::config::Settings;
use crate::core::error::ApiError;

#[utoipa::path(post, path = "/api/auth/register", tag = "auth",
    request_body = Credentials,
    responses((status = 201, body = super::TokenResponse),
              (status = 409, body = crate::core::error::ErrorBody),
              (status = 422, body = crate::core::error::ErrorBody)))]
pub(crate) async fn register(
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<Credentials>,
) -> Result<Response, ApiError> {
    let user = auth::register(&pool, &req.email, &req.password).await?;
    auth::verify_email::send_verification(&pool, &settings.public_url, &user).await?;
    let session = sessions::issue(
        &pool,
        &settings.jwt_secret,
        &user,
        client_of(&headers).as_deref(),
    )
    .await?;
    Ok(session_response(
        &settings,
        jar,
        session,
        user.to_user(),
        StatusCode::CREATED,
    ))
}

#[utoipa::path(post, path = "/api/auth/login", tag = "auth",
    request_body = Credentials,
    responses((status = 200, body = LoginResponse),
              (status = 401, body = crate::core::error::ErrorBody)))]
pub(crate) async fn login(
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<Credentials>,
) -> Result<Response, ApiError> {
    match auth::login(&pool, &req.email, &req.password).await? {
        LoginOutcome::Done(user) => {
            let session = sessions::issue(
                &pool,
                &settings.jwt_secret,
                &user,
                client_of(&headers).as_deref(),
            )
            .await?;
            let jar = jar.add(refresh_cookie(
                &settings,
                session.refresh_token,
                sessions::REFRESH_TTL_DAYS,
            ));
            Ok((
                jar,
                Json(LoginResponse {
                    access_token: Some(session.access_token),
                    user: Some(user.to_user()),
                    requires_2fa: false,
                    pending_token: None,
                }),
            )
                .into_response())
        }
        LoginOutcome::Requires2fa(user_id) => Ok(Json(LoginResponse {
            access_token: None,
            user: None,
            requires_2fa: true,
            pending_token: Some(jwt::sign_pending_2fa(&settings.jwt_secret, user_id)),
        })
        .into_response()),
    }
}

#[derive(Deserialize, ToSchema)]
pub struct Verify2faRequest {
    pub pending_token: String,
    pub code: String,
}

#[utoipa::path(post, path = "/api/auth/2fa/verify", tag = "auth",
    request_body = Verify2faRequest,
    responses((status = 200, body = super::TokenResponse), (status = 401), (status = 422)))]
pub(crate) async fn verify_2fa(
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(req): Json<Verify2faRequest>,
) -> Result<Response, ApiError> {
    let claims = jwt::verify_pending_2fa(&settings.jwt_secret, &req.pending_token)
        .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))?;
    let user = auth::verify_2fa_login(&pool, claims.sub, &req.code).await?;
    let session = sessions::issue(
        &pool,
        &settings.jwt_secret,
        &user,
        client_of(&headers).as_deref(),
    )
    .await?;
    Ok(session_response(
        &settings,
        jar,
        session,
        user.to_user(),
        StatusCode::OK,
    ))
}

#[utoipa::path(post, path = "/api/auth/refresh", tag = "auth",
    responses((status = 200, body = super::TokenResponse), (status = 401)))]
pub(crate) async fn refresh(
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let raw = jar
        .get(REFRESH_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| ApiError::unauthorized("error-unauthorized"))?;
    let (session, user) = sessions::refresh(
        &pool,
        &settings.jwt_secret,
        &raw,
        client_of(&headers).as_deref(),
    )
    .await?;
    Ok(session_response(
        &settings,
        jar,
        session,
        user.to_user(),
        StatusCode::OK,
    ))
}

#[utoipa::path(post, path = "/api/auth/logout", tag = "auth", responses((status = 204)))]
pub(crate) async fn logout(
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    if let Some(cookie) = jar.get(REFRESH_COOKIE) {
        sessions::logout(&pool, cookie.value()).await?;
    }
    let jar = jar.remove(refresh_cookie(&settings, String::new(), 0));
    Ok((StatusCode::NO_CONTENT, jar).into_response())
}
