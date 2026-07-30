//! Фоновые задачи на Postgres — без Redis и брокеров. Воркер живёт в том же
//! процессе, что и api; конкурентные экземпляры не мешают друг другу
//! благодаря `FOR UPDATE SKIP LOCKED`.
//!
//! Задач две: доставка писем из outbox по SMTP (только если задан SMTP_URL)
//! и периодическая уборка — протухшие токены и старая отправленная почта.
//!
//! Лежит не в `core`, а рядом с ним: планировщик знает о доменах (зовёт
//! `auth::cleanup_expired_tokens`), а `core` о доменах знать не должен.
// ponytail: универсальной таблицы jobs с payload'ами нет — заведём, когда
// появится первая настоящая отложенная задача, а не «на всякий случай».

use lettre::message::Mailbox;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::watch;

use crate::core::config::Settings;

const MAX_SEND_ATTEMPTS: i32 = 5;
const TICK: Duration = Duration::from_secs(5);
const CLEANUP_EVERY_TICKS: u32 = 720; // раз в час при тике 5 секунд

/// Запустить воркер. SMTP_URL не задан — письма остаются в outbox (dev-режим).
/// `shutdown` — канал завершения: после сигнала новая пачка не берётся.
pub fn spawn(pool: PgPool, settings: &Settings, mut shutdown: watch::Receiver<bool>) {
    let smtp = match &settings.smtp_url {
        Some(url) => match AsyncSmtpTransport::<Tokio1Executor>::from_url(url.expose()) {
            Ok(builder) => Some(builder.build()),
            Err(err) => {
                tracing::error!(error = %err, "bad SMTP_URL, email delivery disabled");
                None
            }
        },
        None => None,
    };
    let from = settings.smtp_from.clone();
    tokio::spawn(async move {
        // адрес проверен при чтении конфигурации, но полагаться на это нельзя
        let Ok(from) = from.parse::<Mailbox>() else {
            tracing::error!(from, "bad SMTP_FROM, email delivery disabled");
            return;
        };
        let mut tick: u32 = 0;
        while !*shutdown.borrow() {
            if let Some(smtp) = &smtp
                && let Err(err) = deliver_outbox(&pool, smtp, &from).await
            {
                tracing::error!(error = ?err, "outbox delivery failed");
            }
            if tick.is_multiple_of(CLEANUP_EVERY_TICKS) {
                // токены чистит их хозяин — домен auth
                if let Err(err) = crate::auth::cleanup_expired_tokens(&pool).await {
                    tracing::error!(error = ?err, "token cleanup failed");
                }
                if let Err(err) = cleanup_sent_emails(&pool).await {
                    tracing::error!(error = ?err, "sent email cleanup failed");
                }
            }
            tick = tick.wrapping_add(1);
            tokio::select! {
                () = tokio::time::sleep(TICK) => {}
                _ = shutdown.changed() => break,
            }
        }
        tracing::info!("background worker stopped");
    });
}

/// Отправить пачку неотправленных писем. Возвращает число доставленных.
/// Ошибка доставки не роняет пачку: attempts растёт, после лимита письмо
/// больше не берётся (остаётся в таблице для разбора руками).
pub async fn deliver_outbox<T: AsyncTransport + Sync>(
    pool: &PgPool,
    smtp: &T,
    from: &Mailbox,
) -> anyhow::Result<u32>
where
    T::Error: std::error::Error + Send + Sync + 'static,
{
    let mut tx = pool.begin().await?;
    let batch: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT id, recipient, subject, body FROM outbox_emails
         WHERE sent_at IS NULL AND attempts < $1
         ORDER BY id LIMIT 20 FOR UPDATE SKIP LOCKED",
    )
    .bind(MAX_SEND_ATTEMPTS)
    .fetch_all(&mut *tx)
    .await?;
    let mut delivered = 0;
    for (id, recipient, subject, body) in batch {
        let sent = match recipient.parse::<Mailbox>() {
            Ok(to) => {
                let message = build_message(from.clone(), to, subject, body)?;
                match smtp.send(message).await {
                    Ok(_) => true,
                    Err(err) => {
                        tracing::warn!(id, error = %err, "email send failed, will retry");
                        false
                    }
                }
            }
            // кривой адрес не станет лучше от повторов — сжигаем попытки сразу
            Err(err) => {
                tracing::warn!(id, error = %err, "bad recipient address, dropping");
                sqlx::query("UPDATE outbox_emails SET attempts = $2 WHERE id = $1")
                    .bind(id)
                    .bind(MAX_SEND_ATTEMPTS)
                    .execute(&mut *tx)
                    .await?;
                continue;
            }
        };
        if sent {
            delivered += 1;
            sqlx::query("UPDATE outbox_emails SET sent_at = now() WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE outbox_emails SET attempts = attempts + 1 WHERE id = $1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    if delivered > 0 {
        metrics::counter!("emails_delivered_total").increment(delivered as u64);
    }
    Ok(delivered)
}

/// Собрать письмо. ContentType выставляем сами: lettre без него не пишет
/// charset, и русский текст превращается у получателя в кракозябры.
fn build_message(
    from: Mailbox,
    to: Mailbox,
    subject: String,
    body: String,
) -> anyhow::Result<Message> {
    Ok(Message::builder()
        .from(from)
        .to(to)
        .subject(subject)
        .header(lettre::message::header::ContentType::TEXT_PLAIN)
        .body(body)?)
}

/// Письмо, которое исчерпало попытки и больше не берётся воркером
pub struct FailedEmail {
    pub id: i64,
    pub recipient: String,
    pub subject: String,
    pub attempts: i32,
}

/// Что застряло в очереди — для разбора руками (см. bin/ops.rs)
pub async fn failed_emails(pool: &PgPool) -> sqlx::Result<Vec<FailedEmail>> {
    sqlx::query_as(
        "SELECT id, recipient, subject, attempts FROM outbox_emails
         WHERE sent_at IS NULL AND attempts >= $1 ORDER BY id",
    )
    .bind(MAX_SEND_ATTEMPTS)
    .fetch_all(pool)
    .await
    .map(|rows: Vec<(i64, String, String, i32)>| {
        rows.into_iter()
            .map(|(id, recipient, subject, attempts)| FailedEmail {
                id,
                recipient,
                subject,
                attempts,
            })
            .collect()
    })
}

/// Вернуть письмо в очередь: обнулить счётчик попыток. false — нет такого
/// неотправленного письма.
pub async fn retry_email(pool: &PgPool, id: i64) -> sqlx::Result<bool> {
    let result =
        sqlx::query("UPDATE outbox_emails SET attempts = 0 WHERE id = $1 AND sent_at IS NULL")
            .bind(id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

/// Сколько дней хранить уже отправленные письма (потом это просто балласт)
const KEEP_SENT_DAYS: i32 = 30;

/// Выбросить старую отправленную почту, чтобы очередь не росла вечно
pub async fn cleanup_sent_emails(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM outbox_emails WHERE sent_at < now() - make_interval(days => $1)")
        .bind(KEEP_SENT_DAYS)
        .execute(pool)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    /// Регресс на кракозябры: в письме обязан быть объявлен utf-8
    #[test]
    fn message_declares_utf8_charset() {
        let message = super::build_message(
            "dev@localhost.local".parse().unwrap(),
            "to@example.com".parse().unwrap(),
            "Тема".into(),
            "Привет, мир".into(),
        )
        .unwrap();
        let raw = String::from_utf8(message.formatted()).unwrap();
        assert!(raw.contains("charset=utf-8"), "нет charset: {raw}");
    }
}
