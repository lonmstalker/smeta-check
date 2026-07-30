//! Цикл разбора смет. Фоновый воркер (`jobs.rs`) зовёт `run_pending` по тику,
//! вся логика — здесь: домен сам знает, как разбирать свои сметы.
//!
//! Очередь живёт в самой таблице `estimates` — отдельной таблицы задач нет,
//! потому что задача пока одна. Заявка на работу (claim) — короткий UPDATE со
//! `SKIP LOCKED`: транзакция закрывается сразу, а не держится всё время
//! разбора, иначе долгий файл занимал бы соединение с базой на минуты.

use std::path::Path;

use sqlx::PgPool;
use uuid::Uuid;

use crate::core::storage;
use crate::estimates::{self, parse};

/// Сколько раз пробуем разобрать смету, прежде чем признать поражение
const MAX_ATTEMPTS: i32 = 3;

/// Через сколько минут «разбирается» считается зависшим и берётся заново.
/// Так смета доживает до конца, даже если процесс убили посреди разбора.
const STALE_AFTER_MINUTES: i32 = 10;

/// Сколько смет разбираем за один тик: остальные подождут следующего
const BATCH: usize = 5;

/// Разобрать очередь. Возвращает, сколько смет обработано за этот заход.
pub async fn run_pending(pool: &PgPool, files_dir: &Path) -> anyhow::Result<usize> {
    let mut done = 0;
    while done < BATCH {
        let Some(job) = claim(pool).await? else { break };
        run_one(pool, files_dir, &job).await;
        done += 1;
    }
    Ok(done)
}

struct Job {
    id: Uuid,
    file_ext: String,
    attempts: i32,
}

/// Взять одну смету в работу. Берём и новые, и зависшие: строка в статусе
/// `parsing` дольше `STALE_AFTER_MINUTES` — это след убитого процесса.
async fn claim(pool: &PgPool) -> sqlx::Result<Option<Job>> {
    let row: Option<(Uuid, String, i32)> = sqlx::query_as(
        "UPDATE estimates SET status = 'parsing', attempts = attempts + 1,
                              parsing_started_at = now()
         WHERE id = (
             SELECT id FROM estimates
             WHERE attempts < $1
               AND (status = 'uploaded'
                    OR (status = 'parsing'
                        AND parsing_started_at < now() - make_interval(mins => $2)))
             ORDER BY created_at
             FOR UPDATE SKIP LOCKED
             LIMIT 1
         )
         RETURNING id, file_ext, attempts",
    )
    .bind(MAX_ATTEMPTS)
    .bind(STALE_AFTER_MINUTES)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id, file_ext, attempts)| Job {
        id,
        file_ext,
        attempts,
    }))
}

/// Разобрать одну смету. Ошибки не пробрасываем: одна сломанная смета не
/// должна останавливать очередь — она либо помечается failed, либо ждёт
/// следующей попытки.
async fn run_one(pool: &PgPool, files_dir: &Path, job: &Job) {
    let name = estimates::stored_name(job.id, &job.file_ext);
    let bytes = match storage::read(files_dir, &name).await {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::error!(estimate = %job.id, error = %err, "файл сметы не читается");
            return retry_or_give_up(pool, job).await;
        }
    };
    // разбор — синхронный и тяжёлый: держать его в асинхронном потоке нельзя
    let parsed = match tokio::task::spawn_blocking(move || parse::parse(bytes)).await {
        Ok(result) => result,
        Err(err) => {
            tracing::error!(estimate = %job.id, error = %err, "разбор сорвался");
            return retry_or_give_up(pool, job).await;
        }
    };
    match parsed {
        Ok(lines) => {
            let recognized = lines.iter().filter(|line| line.is_recognized()).count();
            if let Err(err) = estimates::replace_lines(pool, job.id, &lines).await {
                tracing::error!(estimate = %job.id, error = %err, "строки не сохранились");
                return retry_or_give_up(pool, job).await;
            }
            if let Err(err) = mark_parsed(pool, job.id).await {
                tracing::error!(estimate = %job.id, error = %err, "статус не обновился");
                return;
            }
            metrics::counter!("estimates_parsed_total").increment(1);
            tracing::info!(
                estimate = %job.id,
                lines = lines.len(),
                recognized,
                "смета разобрана"
            );
        }
        // файл прочитать не удалось — повторы не помогут, это свойство файла
        Err(reason) => {
            if let Err(err) = mark_failed(pool, job.id, reason.key()).await {
                tracing::error!(estimate = %job.id, error = %err, "статус не обновился");
                return;
            }
            metrics::counter!("estimates_parse_failed_total", "reason" => reason.reason())
                .increment(1);
            tracing::info!(estimate = %job.id, reason = reason.reason(), "смету не разобрали");
        }
    }
}

/// Сбой не из-за файла (диск, база): вернуть смету в очередь, а когда попытки
/// кончились — честно сказать пользователю, что не получилось.
async fn retry_or_give_up(pool: &PgPool, job: &Job) {
    let result = if job.attempts >= MAX_ATTEMPTS {
        metrics::counter!("estimates_parse_failed_total", "reason" => "internal").increment(1);
        mark_failed(pool, job.id, "error-estimate-unreadable").await
    } else {
        sqlx::query("UPDATE estimates SET status = 'uploaded' WHERE id = $1")
            .bind(job.id)
            .execute(pool)
            .await
            .map(|_| ())
    };
    if let Err(err) = result {
        tracing::error!(estimate = %job.id, error = %err, "не удалось вернуть смету в очередь");
    }
}

async fn mark_parsed(pool: &PgPool, id: Uuid) -> sqlx::Result<()> {
    sqlx::query("UPDATE estimates SET status = 'parsed', error_key = NULL WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map(|_| ())
}

async fn mark_failed(pool: &PgPool, id: Uuid, key: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE estimates SET status = 'failed', error_key = $2 WHERE id = $1")
        .bind(id)
        .bind(key)
        .execute(pool)
        .await
        .map(|_| ())
}
