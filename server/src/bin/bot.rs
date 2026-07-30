//! Телеграм-бот — второй транспорт над той же доменной библиотекой.
//! ponytail: заглушка; когда бот понадобится — `cargo add teloxide` и
//! хендлеры зовут server::items::* напрямую (без HTTP-хопа и без второй БД).

#[expect(
    clippy::print_stdout,
    reason = "у консольной заглушки stdout — единственный интерфейс"
)]
fn main() {
    println!("bot: not implemented yet; add teloxide and call server::items::*");
}
