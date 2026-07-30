//! Загрузка и просмотр смет. Сметы личные: список и карточка приходят только
//! владельцу, чужая смета отвечает так же, как несуществующая.

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use sqlx::PgPool;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;
use crate::core::config::Settings;
use crate::core::error::ApiError;
use crate::core::storage;
use crate::estimates::{self, Estimate};
use crate::users::http::CurrentUser;

/// Файл идёт по мобильному интернету дольше, чем короткий запрос: общих
/// 15 секунд не хватит, поэтому у загрузки свой таймаут.
const UPLOAD_TIMEOUT: Duration = Duration::from_secs(60);

/// Потолок тела чуть выше потолка файла: в теле есть ещё границы и заголовки
/// multipart. Файл ровно в 10 МиБ обязан загрузиться, а вот тело в 20 МиБ
/// отсекается сразу, не доходя до разбора.
const UPLOAD_BODY_LIMIT_BYTES: usize = estimates::MAX_FILE_BYTES + 64 * 1024;

/// Чтение смет: обычные маршруты с общими лимитами приложения
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/estimates", get(list_estimates))
        .route("/api/estimates/{id}", get(get_estimate))
}

/// Загрузка живёт отдельным роутером, потому что у неё всё своё: потолок тела
/// (файл, а не JSON), таймаут и лимит частоты. Общие слои приложения на него
/// не навешиваются — иначе 15-секундный таймаут обрывал бы загрузку.
pub fn upload_router(settings: &Settings) -> Router<AppState> {
    let router = Router::new()
        .route("/api/estimates", post(upload_estimate))
        .layer(DefaultBodyLimit::max(UPLOAD_BODY_LIMIT_BYTES))
        .layer(tower_http::timeout::TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            UPLOAD_TIMEOUT,
        ));
    // без лимита частоты один скрипт зальёт диск быстрее, чем мы заметим
    crate::core::rate_limit::limit_per_ip(
        router,
        settings.rate_limit_upload_rpm,
        settings.trust_proxy,
    )
}

/// Тело загрузки описано руками: генератор спеки не умеет выводить
/// multipart с файлом из сигнатуры хендлера.
#[derive(ToSchema)]
pub struct UploadForm {
    /// файл сметы: xlsx или xls, до 10 МиБ
    #[schema(value_type = String, format = Binary)]
    pub file: String,
}

#[utoipa::path(post, path = "/api/estimates", tag = "estimates",
    request_body(content = UploadForm, content_type = "multipart/form-data"),
    responses((status = 201, body = Estimate), (status = 401),
              (status = 413, body = crate::core::error::ErrorBody),
              (status = 422, body = crate::core::error::ErrorBody),
              (status = 429, body = crate::core::error::ErrorBody)),
    security(("bearer" = [])))]
pub(crate) async fn upload_estimate(
    user: CurrentUser,
    State(pool): State<PgPool>,
    State(settings): State<Arc<Settings>>,
    mut form: Multipart,
) -> Result<(StatusCode, Json<Estimate>), ApiError> {
    if estimates::count_of(&pool, user.id).await? >= estimates::MAX_PER_USER {
        return Err(
            ApiError::validation("error-estimate-limit").arg("max", estimates::MAX_PER_USER)
        );
    }
    let field = form
        .next_field()
        .await
        .map_err(|err| {
            tracing::warn!(error = %err, "не разобрали multipart загрузки");
            no_file()
        })?
        .ok_or_else(no_file)?;
    let file_name = estimates::clean_file_name(field.file_name().unwrap_or_default());
    let Some(ext) = estimates::extension_of(&file_name) else {
        return Err(ApiError::validation("error-estimate-format").field("file"));
    };
    let bytes = field.bytes().await.map_err(|err| {
        tracing::warn!(error = %err, "файл сметы не дочитался");
        no_file()
    })?;
    if bytes.is_empty() {
        return Err(ApiError::validation("error-estimate-empty").field("file"));
    }
    if bytes.len() > estimates::MAX_FILE_BYTES {
        return Err(ApiError::too_large("error-estimate-too-large")
            .arg("max", estimates::MAX_FILE_BYTES / (1024 * 1024))
            .field("file"));
    }

    let id = Uuid::new_v4();
    // сначала файл, потом запись: осиротевший файл — это мусор на диске, а
    // карточка без файла — смета, которую невозможно разобрать
    storage::save(
        &settings.files_dir,
        &estimates::stored_name(id, ext),
        &bytes,
    )
    .await?;
    let estimate = estimates::create(
        &pool,
        id,
        user.id,
        &file_name,
        ext,
        bytes.len().try_into().unwrap_or(i64::MAX),
    )
    .await?;
    metrics::counter!("estimates_uploaded_total").increment(1);
    Ok((StatusCode::CREATED, Json(estimate)))
}

fn no_file() -> ApiError {
    ApiError::validation("error-estimate-no-file").field("file")
}

#[utoipa::path(get, path = "/api/estimates", tag = "estimates",
    responses((status = 200, body = Vec<Estimate>), (status = 401)),
    security(("bearer" = [])))]
pub(crate) async fn list_estimates(
    user: CurrentUser,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<Estimate>>, ApiError> {
    Ok(Json(estimates::list(&pool, user.id).await?))
}

#[utoipa::path(get, path = "/api/estimates/{id}", tag = "estimates",
    responses((status = 200, body = Estimate), (status = 401), (status = 404)),
    security(("bearer" = [])))]
pub(crate) async fn get_estimate(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<Estimate>, ApiError> {
    estimates::get(&pool, user.id, id)
        .await?
        .map(Json)
        .ok_or_else(ApiError::not_found)
}
