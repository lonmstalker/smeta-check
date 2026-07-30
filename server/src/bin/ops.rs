//! Операторские команды: то, что иногда нужно сделать руками на сервере.
//!
//! Без clap: три команды разбираются десятком строк, а новая зависимость — это
//! ещё один источник обновлений и уязвимостей. Логика не своя — те же функции
//! доменов, что зовёт HTTP-слой.
//!
//! На сервере: docker compose -f compose.prod.yaml exec app ops <команда>

use server::core::config::Settings;
use server::core::db;
use server::jobs;
use server::users::{self, Role};

const USAGE: &str = "\
Команды:
  config check              проверить, что конфигурация читается и валидна
  promote-admin <email>     выдать пользователю роль администратора
  outbox list-failed        письма, которые перестали отправляться
  outbox retry <id>         вернуть письмо в очередь отправки";

#[tokio::main]
#[expect(
    clippy::print_stdout,
    reason = "у консольной команды stdout — это и есть интерфейс"
)]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    // конфигурация читается для любой команды: половина из них ходит в БД,
    // а `config check` только этим и занимается
    let settings = Settings::from_env()?;

    match args.as_slice() {
        ["config", "check"] => {
            println!("конфигурация прочитана и проверена");
            Ok(())
        }
        ["promote-admin", email] => {
            let pool = db::connect(settings.database_url.expose()).await?;
            if !users::set_role_by_email(&pool, email, Role::Admin).await? {
                anyhow::bail!("пользователь {email} не найден");
            }
            println!("{email} теперь администратор (роль обновится при следующем входе)");
            Ok(())
        }
        ["outbox", "list-failed"] => {
            let pool = db::connect(settings.database_url.expose()).await?;
            let failed = jobs::failed_emails(&pool).await?;
            if failed.is_empty() {
                println!("застрявших писем нет");
            }
            for email in failed {
                println!(
                    "{}\t{}\t{}\tпопыток: {}",
                    email.id, email.recipient, email.subject, email.attempts
                );
            }
            Ok(())
        }
        ["outbox", "retry", id] => {
            let id: i64 = id
                .parse()
                .map_err(|_| anyhow::anyhow!("id письма — это число, а не {id:?}"))?;
            let pool = db::connect(settings.database_url.expose()).await?;
            if !jobs::retry_email(&pool, id).await? {
                anyhow::bail!("неотправленного письма с id {id} нет");
            }
            println!("письмо {id} вернулось в очередь");
            Ok(())
        }
        _ => {
            println!("{USAGE}");
            std::process::exit(2);
        }
    }
}
