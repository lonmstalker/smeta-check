//! Отправка писем. Сейчас один режим — outbox: письмо кладётся в таблицу
//! outbox_emails (тесты и dev читают его оттуда) и пишется в лог.
//! ponytail: перед продом добавить SMTP (lettre) здесь же — это единственная
//! точка отправки во всём коде.

use sqlx::PgPool;

pub async fn send(pool: &PgPool, to: &str, subject: &str, body: &str) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO outbox_emails (recipient, subject, body) VALUES ($1, $2, $3)")
        .bind(to)
        .bind(subject)
        .bind(body)
        .execute(pool)
        .await?;
    tracing::info!(to, subject, "email queued (dev outbox)");
    Ok(())
}
