//! Время в ответах API — всегда RFC 3339 в UTC. Переводить его в местный
//! формат — работа браузера: на сервере часового пояса пользователя нет.

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_default()
}
