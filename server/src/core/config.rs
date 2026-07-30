//! Вся конфигурация процесса — в одном месте. Читается и проверяется один
//! раз до старта HTTP: неверная настройка = процесс не поднялся, а не «упал
//! на первом запросе через неделю».
//!
//! Правило: `std::env::var` живёт только здесь (скрипт check-suppressions
//! следит). Домены и хендлеры получают готовый `Settings`.

use std::collections::BTreeMap;
use std::net::SocketAddr;

/// Строка, которую нельзя случайно вывести в лог: `Debug` печатает звёздочки.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

/// Значение внутрь кладут конфигурация и тесты; наружу — только expose()
impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\"***\"")
    }
}

/// Внешний провайдер входа (VK ID, Яндекс ID, ...) — чистая конфигурация,
/// подключается переменными окружения без изменения кода (docs/oauth.md).
#[derive(Debug, Clone)]
pub struct OauthProvider {
    pub name: String,
    pub client_id: String,
    pub client_secret: Secret,
    pub auth_url: String,
    pub token_url: String,
    pub userinfo_url: String,
    pub scope: String,
    /// JSON-указатель на id в ответе профиля, например "/user/user_id" у VK ID
    pub id_pointer: String,
    pub email_pointer: Option<String>,
    /// Провайдер сам проверяет адрес почты (VK ID, Яндекс ID — да). Только с
    /// этим флагом вход по email прицепляется к существующему аккаунту;
    /// иначе чужой «непроверенный» адрес был бы угоном чужой учётки.
    pub trust_email: bool,
}

#[derive(Debug)]
pub struct LogSettings {
    /// LOG_FORMAT=json — структурные логи для прода
    pub json: bool,
    /// LOG_DIR — дублировать поток в файлы с ротацией по дням
    pub dir: Option<String>,
}

#[derive(Debug)]
pub struct Settings {
    pub database_url: Secret,
    pub port: u16,
    pub jwt_secret: Secret,
    /// базовый адрес приложения: ссылки в письмах и OAuth-callback
    pub public_url: String,
    /// refresh-cookie только по https (прод)
    pub cookie_secure: bool,
    /// доверять X-Forwarded-For (только если перед приложением есть наш прокси)
    pub trust_proxy: bool,
    pub cors_origins: Vec<String>,
    /// запросов в минуту с одного IP на auth-ручки; 0 — без ограничения
    pub rate_limit_auth_rpm: u32,
    /// то же для /api/logs: ошибки фронта шлёт и гость, вход не требуется
    pub rate_limit_logs_rpm: u32,
    /// то же для загрузки смет: файл дорог и по диску, и по разбору
    pub rate_limit_upload_rpm: u32,
    /// каталог, где лежат файлы смет (в проде — volume, в тестах — временный)
    pub files_dir: std::path::PathBuf,
    /// как часто фоновый воркер заглядывает в очереди; на стенде e2e короче,
    /// иначе браузер ждёт разбора сметы дольше, чем длится сам сценарий
    pub worker_tick: std::time::Duration,
    pub smtp_url: Option<Secret>,
    pub smtp_from: String,
    pub metrics_addr: SocketAddr,
    pub log: LogSettings,
    pub oauth: BTreeMap<String, OauthProvider>,
}

/// Минимальная длина секрета подписи: короче — не секрет, а опечатка
const MIN_JWT_SECRET_LEN: usize = 16;

impl Settings {
    /// Прочитать и проверить окружение. Ошибка = приложение не стартует.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = required("DATABASE_URL")?;
        let jwt_secret = required("JWT_SECRET")?;
        check_jwt_secret(&jwt_secret)?;
        let smtp_from = var("SMTP_FROM").unwrap_or_else(|| "no-reply@localhost".into());
        smtp_from
            .parse::<lettre::message::Mailbox>()
            .map_err(|e| anyhow::anyhow!("SMTP_FROM не похож на почтовый адрес: {e}"))?;

        Ok(Self {
            database_url: Secret(database_url),
            port: parsed("PORT", 8080)?,
            jwt_secret: Secret(jwt_secret),
            public_url: var("PUBLIC_URL").unwrap_or_else(|| "http://localhost:5173".into()),
            cookie_secure: flag("COOKIE_SECURE"),
            trust_proxy: flag("TRUST_PROXY"),
            cors_origins: var("CORS_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|o| !o.is_empty())
                .map(str::to_owned)
                .collect(),
            rate_limit_auth_rpm: parsed("RATE_LIMIT_AUTH_RPM", 30)?,
            rate_limit_logs_rpm: parsed("RATE_LIMIT_LOGS_RPM", 60)?,
            rate_limit_upload_rpm: parsed("RATE_LIMIT_UPLOAD_RPM", 10)?,
            files_dir: var("FILES_DIR").map_or_else(|| "./data/files".into(), Into::into),
            worker_tick: std::time::Duration::from_secs(parsed("WORKER_TICK_SECS", 5)?),
            smtp_url: var("SMTP_URL").filter(|u| !u.is_empty()).map(Secret),
            smtp_from,
            metrics_addr: parsed("METRICS_ADDR", "127.0.0.1:9464".parse()?)?,
            log: LogSettings {
                json: var("LOG_FORMAT").is_some_and(|v| v == "json"),
                dir: var("LOG_DIR"),
            },
            oauth: oauth_providers()?,
        })
    }

    /// Настройки для тестов и локальных инструментов: всё безопасное,
    /// секрет подписи — заведомо не прод.
    pub fn for_tests() -> Self {
        Self {
            database_url: Secret(String::new()),
            port: 0,
            jwt_secret: Secret("test-secret-not-for-prod".into()),
            public_url: "http://localhost:5173".into(),
            cookie_secure: false,
            trust_proxy: true,
            cors_origins: Vec::new(),
            rate_limit_auth_rpm: 30,
            rate_limit_logs_rpm: 60,
            rate_limit_upload_rpm: 0,
            // тесты подставляют сюда свой временный каталог (см. spawn_app)
            files_dir: std::env::temp_dir().join("smeta-check-tests"),
            worker_tick: std::time::Duration::from_millis(50),
            smtp_url: None,
            smtp_from: "no-reply@localhost".into(),
            metrics_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            log: LogSettings {
                json: false,
                dir: None,
            },
            oauth: BTreeMap::new(),
        }
    }
}

fn check_jwt_secret(secret: &str) -> anyhow::Result<()> {
    if secret.chars().count() < MIN_JWT_SECRET_LEN {
        anyhow::bail!(
            "JWT_SECRET короче {MIN_JWT_SECRET_LEN} символов — сгенерируй новый: openssl rand -hex 32"
        );
    }
    Ok(())
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn required(name: &str) -> anyhow::Result<String> {
    var(name).ok_or_else(|| anyhow::anyhow!("не задана обязательная переменная {name}"))
}

fn flag(name: &str) -> bool {
    var(name).is_some_and(|v| v == "true" || v == "1")
}

fn parsed<T: std::str::FromStr>(name: &str, default: T) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    match var(name) {
        Some(raw) => raw
            .parse()
            .map_err(|e| anyhow::anyhow!("{name}={raw} не разбирается: {e}")),
        None => Ok(default),
    }
}

/// Провайдеров ищем по самим переменным: есть OAUTH_<ИМЯ>_CLIENT_ID —
/// значит провайдер `имя` подключён, остальные его поля обязаны быть рядом.
fn oauth_providers() -> anyhow::Result<BTreeMap<String, OauthProvider>> {
    let names: Vec<String> = std::env::vars()
        .filter_map(|(key, _)| {
            key.strip_prefix("OAUTH_")
                .and_then(|rest| rest.strip_suffix("_CLIENT_ID"))
                .map(str::to_ascii_lowercase)
        })
        .collect();
    let mut providers = BTreeMap::new();
    for name in names {
        let upper = name.to_ascii_uppercase();
        let field = |suffix: &str| required(&format!("OAUTH_{upper}_{suffix}"));
        let auth_url = field("AUTH_URL")?;
        // ссылку на согласие мы потом достраиваем параметрами — она обязана
        // быть разбираемым URL уже сейчас, а не в момент первого входа
        reqwest::Url::parse(&auth_url)
            .map_err(|e| anyhow::anyhow!("OAUTH_{upper}_AUTH_URL не разбирается: {e}"))?;
        providers.insert(
            name.clone(),
            OauthProvider {
                name,
                client_id: field("CLIENT_ID")?,
                client_secret: Secret(field("CLIENT_SECRET")?),
                auth_url,
                token_url: field("TOKEN_URL")?,
                userinfo_url: field("USERINFO_URL")?,
                scope: var(&format!("OAUTH_{upper}_SCOPE")).unwrap_or_default(),
                id_pointer: var(&format!("OAUTH_{upper}_ID_POINTER"))
                    .unwrap_or_else(|| "/id".into()),
                email_pointer: var(&format!("OAUTH_{upper}_EMAIL_POINTER")),
                trust_email: flag(&format!("OAUTH_{upper}_TRUST_EMAIL")),
            },
        );
    }
    Ok(providers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_is_not_printed() {
        let settings = Settings::for_tests();
        let dump = format!("{settings:?}");
        assert!(
            !dump.contains("test-secret-not-for-prod"),
            "секрет утёк в Debug: {dump}"
        );
    }

    #[test]
    fn short_jwt_secret_is_rejected() {
        assert!(check_jwt_secret("короткий").is_err());
        assert!(check_jwt_secret(&"a".repeat(MIN_JWT_SECRET_LEN)).is_ok());
    }
}
