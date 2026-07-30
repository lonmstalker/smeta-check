//! Локализация ответов API: Fluent-словари ru/en с правильными
//! множественными формами. Язык запроса кладётся в task-local middleware'ом,
//! поэтому хендлеры и ошибки не передают его вручную.

use std::collections::HashMap;
use std::sync::OnceLock;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::{LanguageIdentifier, langid};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Lang {
    Ru,
    En,
}

/// Порядок = приоритет; первый — язык по умолчанию.
/// Новый язык: добавить вариант сюда, файл в server/locales/<код>/main.ftl
/// и словарь на фронте — тесты подскажут, если что-то забыто.
pub const ALL_LANGS: &[(Lang, &str)] = &[(Lang::Ru, "ru"), (Lang::En, "en")];

impl Lang {
    pub fn code(self) -> &'static str {
        // ALL_LANGS перечисляет все варианты; фолбэк — язык по умолчанию
        ALL_LANGS
            .iter()
            .find(|(l, _)| *l == self)
            .map_or(ALL_LANGS[0].1, |(_, code)| *code)
    }

    fn from_accept_language(header: &str) -> Self {
        // достаточно первого поддерживаемого кода в порядке предпочтений клиента
        for part in header.split(',') {
            let code = part
                .split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            for (lang, prefix) in ALL_LANGS {
                if code.starts_with(prefix) {
                    return *lang;
                }
            }
        }
        ALL_LANGS[0].0
    }
}

fn locale_id(lang: Lang) -> LanguageIdentifier {
    match lang {
        Lang::Ru => langid!("ru"),
        Lang::En => langid!("en"),
    }
}

// Словари вшиты в бинарь: нет чтения диска и нечему потеряться при деплое
fn source(lang: Lang) -> &'static str {
    match lang {
        Lang::Ru => include_str!("../../locales/ru/main.ftl"),
        Lang::En => include_str!("../../locales/en/main.ftl"),
    }
}

fn bundles() -> &'static HashMap<Lang, FluentBundle<FluentResource>> {
    static BUNDLES: OnceLock<HashMap<Lang, FluentBundle<FluentResource>>> = OnceLock::new();
    BUNDLES.get_or_init(|| {
        ALL_LANGS
            .iter()
            .filter_map(|&(lang, code)| {
                let resource = parse_ftl(lang, code)?;
                let mut bundle = FluentBundle::new_concurrent(vec![locale_id(lang)]);
                if let Err(errors) = bundle.add_resource(resource) {
                    tracing::error!(code, ?errors, "duplicate message ids in .ftl");
                    return None;
                }
                Some((lang, bundle))
            })
            .collect()
    })
}

/// Словарь вшит в бинарь, поэтому сломанный синтаксис — баг сборки, а не
/// внешний сбой: язык выпадает, тесты полноты словарей это сразу показывают.
fn parse_ftl(lang: Lang, code: &str) -> Option<FluentResource> {
    match FluentResource::try_new(source(lang).to_owned()) {
        Ok(resource) => Some(resource),
        Err((_, errors)) => {
            tracing::error!(code, ?errors, "broken .ftl syntax");
            None
        }
    }
}

pub fn translate(lang: Lang, key: &str, args: &[(&'static str, String)]) -> String {
    let Some(bundle) = bundles().get(&lang) else {
        tracing::error!(?lang, "no localization bundle");
        return key.to_owned();
    };
    let Some(message) = bundle.get_message(key).and_then(|m| m.value()) else {
        // отсутствие ключа — баг разработки; тесты сверки ключей это ловят
        tracing::error!(key, "missing localization key");
        return key.to_owned();
    };
    let mut fluent_args = FluentArgs::new();
    for (name, value) in args {
        // числа передаём числами, чтобы работали множественные формы
        match value.parse::<f64>() {
            Ok(num) => fluent_args.set(*name, FluentValue::from(num)),
            Err(_) => fluent_args.set(*name, FluentValue::from(value.clone())),
        }
    }
    let mut errors = Vec::new();
    bundle
        .format_pattern(message, Some(&fluent_args), &mut errors)
        .into_owned()
}

/// Все ключи словаря — для теста «локали не расходятся»
pub fn message_keys(lang: Lang) -> Vec<String> {
    let Some(resource) = parse_ftl(lang, lang.code()) else {
        return Vec::new();
    };
    resource
        .entries()
        .filter_map(|entry| match entry {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_owned()),
            _ => None,
        })
        .collect()
}

tokio::task_local! {
    static LANG: Lang;
}

pub fn current_lang() -> Lang {
    LANG.try_with(|l| *l).unwrap_or(ALL_LANGS[0].0)
}

/// Middleware: определяет язык из Accept-Language на весь запрос
pub async fn lang_middleware(req: Request, next: Next) -> Response {
    let lang = req
        .headers()
        .get("accept-language")
        .and_then(|v| v.to_str().ok())
        .map(Lang::from_accept_language)
        .unwrap_or(ALL_LANGS[0].0);
    LANG.scope(lang, next.run(req)).await
}
