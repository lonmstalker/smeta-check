//! Интеграционные тесты API на реальном Postgres. Один тест-бинарь,
//! внутри — области по доменам; инфраструктура в common.rs.
//!
//! `unwrap/expect` внутри `#[test]`-функций разрешает clippy.toml; для
//! вспомогательных функций вне тестов разрешение точечное, у каждой своё.

#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "тестовая инфраструктура: паника хелпера валит тест — это и есть отчёт об ошибке"
)]
mod common;

mod account;
mod auth;
mod auth_oauth;
mod auth_recovery;
mod auth_totp;
mod auth_verify;
mod contract;
mod estimates;
mod estimates_parse;
mod hardening;
mod i18n;
mod ops;
mod sessions;
