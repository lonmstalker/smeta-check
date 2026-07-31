//! Загрузка и просмотр смет. Сметы личные: список и карточка приходят только
//! владельцу, чужая смета отвечает так же, как несуществующая.

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Multipart, Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use sqlx::PgPool;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;
use crate::core::config::Settings;
use crate::core::error::ApiError;
use crate::core::storage;
use crate::estimates::{self, Estimate, EstimateLine};
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
    /// файл сметы: xlsx, xls или фотография (jpg, png, webp), до 10 МиБ.
    /// Фото принимается только при включённой нейросети и подтверждённой почте
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
    // ранняя проверка — чтобы не писать файл на диск зря; настоящая граница
    // лимита — под замком в `estimates::create`
    if estimates::count_of(&pool, user.id).await? >= estimates::MAX_PER_USER {
        return Err(limit_reached());
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
    if estimates::is_photo(ext) {
        allow_photo(&pool, &settings, user.id).await?;
    }
    let bytes = field.bytes().await.map_err(|err| {
        tracing::warn!(error = %err, "файл сметы не дочитался");
        no_file()
    })?;
    if bytes.is_empty() {
        return Err(ApiError::validation("error-estimate-empty").field("file"));
    }
    // проверяем до записи на диск: мусор, переименованный в .jpg, не должен
    // стоить трёх вызовов нейросети
    if !estimates::photo_matches_ext(ext, &bytes) {
        return Err(ApiError::validation("error-estimate-not-a-photo").field("file"));
    }
    if bytes.len() > estimates::MAX_FILE_BYTES {
        return Err(ApiError::too_large("error-estimate-too-large")
            .arg("max", estimates::MAX_FILE_BYTES / (1024 * 1024))
            .field("file"));
    }

    let id = Uuid::new_v4();
    // сначала файл, потом запись: карточка без файла — смета, которую
    // невозможно разобрать, а не вставшая карточка убирает файл за собой
    let name = estimates::stored_name(id, ext);
    storage::save(&settings.files_dir, &name, &bytes).await?;
    let created = estimates::create(
        &pool,
        id,
        user.id,
        &file_name,
        ext,
        bytes.len().try_into().unwrap_or(i64::MAX),
    )
    .await;
    let estimate = match created {
        Ok(Some(estimate)) => estimate,
        // лимит добит параллельной загрузкой или база отказала — файл на
        // диске осиротел, убираем его же
        Ok(None) => {
            storage::remove(&settings.files_dir, &name).await;
            return Err(limit_reached());
        }
        Err(err) => {
            storage::remove(&settings.files_dir, &name).await;
            return Err(err.into());
        }
    };
    metrics::counter!("estimates_uploaded_total").increment(1);
    Ok((StatusCode::CREATED, Json(estimate)))
}

/// Фото — дорогой вход: каждое стоит вызова нейросети. Поэтому два условия.
/// Первое: нейросеть вообще включена (иначе фото зависло бы в очереди
/// навсегда). Второе: почта подтверждена — бесплатных аккаунтов без
/// настоящей почты у дорогого разбора нет.
async fn allow_photo(pool: &PgPool, settings: &Settings, user_id: Uuid) -> Result<(), ApiError> {
    if !crate::core::llm::enabled(settings) {
        return Err(ApiError::validation("error-estimate-photo-off").field("file"));
    }
    let verified = crate::users::find_by_id(pool, user_id)
        .await?
        .is_some_and(|user| user.email_verified);
    if !verified {
        return Err(ApiError::validation("error-estimate-photo-needs-email").field("file"));
    }
    Ok(())
}

fn no_file() -> ApiError {
    ApiError::validation("error-estimate-no-file").field("file")
}

fn limit_reached() -> ApiError {
    ApiError::validation("error-estimate-limit").arg("max", estimates::MAX_PER_USER)
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

/// Смета со строками. Строки приходят и распознанные, и сырые: нераспознанное
/// показывается блоком «спросите бригаду, что это», а не прячется.
#[derive(Serialize, ToSchema)]
pub struct EstimateDetails {
    #[serde(flatten)]
    pub estimate: Estimate,
    pub lines: Vec<EstimateLine>,
}

#[utoipa::path(get, path = "/api/estimates/{id}", tag = "estimates",
    responses((status = 200, body = EstimateDetails), (status = 401), (status = 404)),
    security(("bearer" = [])))]
pub(crate) async fn get_estimate(
    user: CurrentUser,
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<EstimateDetails>, ApiError> {
    let estimate = estimates::get(&pool, user.id, id)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let lines = estimates::lines_of(&pool, id).await?;
    Ok(Json(EstimateDetails { estimate, lines }))
}
