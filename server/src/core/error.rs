//! Единая ошибка API. Хендлеры возвращают её с КЛЮЧОМ локализации,
//! текст для пользователя подставляется на его языке в момент ответа.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

use crate::core::i18n;

#[derive(Debug)] // тесты зовут доменные функции напрямую: нужен unwrap
pub struct ApiError {
    status: StatusCode,
    /// ключ из server/locales/*/main.ftl; он же машиночитаемый код для фронта
    key: &'static str,
    args: Vec<(&'static str, String)>,
    /// поле формы, к которому относится ошибка (если она вообще про поле)
    field: Option<&'static str>,
    source: Option<anyhow::Error>,
}

impl ApiError {
    fn new(status: StatusCode, key: &'static str) -> Self {
        Self {
            status,
            key,
            args: Vec::new(),
            field: None,
            source: None,
        }
    }

    pub fn validation(key: &'static str) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, key)
    }

    pub fn unauthorized(key: &'static str) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, key)
    }

    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "error-forbidden")
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "error-not-found")
    }

    pub fn conflict(key: &'static str) -> Self {
        Self::new(StatusCode::CONFLICT, key)
    }

    /// Файл больше разрешённого. Отдельный статус нужен, чтобы клиент отличал
    /// «слишком большой» от «неверный формат» без разбора текста.
    pub fn too_large(key: &'static str) -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, key)
    }

    pub fn too_many_requests() -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, "error-too-many-requests")
    }

    /// Подставить значение в текст ошибки, например min для длины пароля
    pub fn arg(mut self, name: &'static str, value: impl ToString) -> Self {
        self.args.push((name, value.to_string()));
        self
    }

    /// Привязать ошибку к полю формы: фронт подсветит именно его, а не покажет
    /// одну строку под всей формой. Имя поля = имя в теле запроса.
    pub fn field(mut self, name: &'static str) -> Self {
        self.field = Some(name);
        self
    }
}

/// Любая непредвиденная ошибка (БД и т.п.) -> 500 без деталей наружу
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            key: "error-internal",
            args: Vec::new(),
            field: None,
            source: Some(err.into()),
        }
    }
}

/// Тело ответа с ошибкой — единственный формат на весь API.
/// `fields` появляется только у ошибок про конкретные поля формы.
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Serialize, ToSchema)]
pub struct ErrorDetail {
    /// машиночитаемый код (он же ключ локализации)
    pub code: String,
    /// текст на языке запроса — можно показывать пользователю как есть
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<Vec<FieldError>>,
}

#[derive(Serialize, ToSchema)]
pub struct FieldError {
    /// имя поля в теле запроса, например "password"
    pub field: String,
    pub code: String,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let Some(err) = &self.source {
            tracing::error!(error = ?err, "request failed");
        }
        let message = i18n::translate(i18n::current_lang(), self.key, &self.args);
        let fields = self.field.map(|field| {
            vec![FieldError {
                field: field.to_owned(),
                code: self.key.to_owned(),
                message: message.clone(),
            }]
        });
        (
            self.status,
            Json(ErrorBody {
                error: ErrorDetail {
                    code: self.key.to_owned(),
                    message,
                    fields,
                },
            }),
        )
            .into_response()
    }
}
