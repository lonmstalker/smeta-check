//! Фоновые задачи и операторские команды: доставка писем из outbox, разбор
//! застрявших писем, чистка протухших токенов, выдача роли администратора.

use lettre::transport::stub::AsyncStubTransport;

use crate::common::{fixture, spawn_app, spawn_app_with};

/// Отправитель для тестов доставки — адрес не важен, важен сам факт письма
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "хелпер теста: паника валит тест — это и есть отчёт об ошибке"
)]
fn test_sender() -> lettre::message::Mailbox {
    "dev@localhost.local".parse().unwrap()
}

#[tokio::test]
async fn outbox_delivery_marks_emails_sent() {
    let app = spawn_app().await;
    server::core::mailer::send(&app.pool, "to@example.com", "Тема", "Текст")
        .await
        .unwrap();

    let smtp = AsyncStubTransport::new_ok();
    let delivered = server::jobs::deliver_outbox(&app.pool, &smtp, &test_sender())
        .await
        .unwrap();
    assert_eq!(delivered, 1);

    // повторный прогон не шлёт то же письмо второй раз
    let delivered = server::jobs::deliver_outbox(&app.pool, &smtp, &test_sender())
        .await
        .unwrap();
    assert_eq!(delivered, 0);
}

#[tokio::test]
async fn outbox_failures_stop_after_max_attempts() {
    let app = spawn_app().await;
    server::core::mailer::send(&app.pool, "to@example.com", "Тема", "Текст")
        .await
        .unwrap();

    let broken = AsyncStubTransport::new_error();
    for _ in 0..6 {
        let delivered = server::jobs::deliver_outbox(&app.pool, &broken, &test_sender())
            .await
            .unwrap();
        assert_eq!(delivered, 0);
    }
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM outbox_emails")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(attempts, 5, "после пяти неудач письмо больше не берётся");
}

#[tokio::test]
async fn cleanup_removes_old_sent_emails_but_keeps_the_queue() {
    let app = spawn_app().await;
    server::core::mailer::send(&app.pool, "to@example.com", "Свежее", "Текст")
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO outbox_emails (recipient, subject, body, sent_at)
         VALUES ('old@example.com', 'Старое', 'Текст', now() - interval '31 days')",
    )
    .execute(&app.pool)
    .await
    .unwrap();

    server::jobs::cleanup_sent_emails(&app.pool).await.unwrap();

    let left: Vec<String> = sqlx::query_scalar("SELECT subject FROM outbox_emails")
        .fetch_all(&app.pool)
        .await
        .unwrap();
    assert_eq!(left, vec!["Свежее".to_string()]);
}

/// Регресс: пока цикл был один, кривой SMTP_FROM обрывал его целиком — и
/// сметы переставали разбираться из-за настройки почты, к которой они не
/// имеют отношения.
#[tokio::test]
async fn broken_smtp_from_does_not_stop_estimate_parsing() {
    let app = spawn_app_with(|settings| settings.smtp_from = "это не адрес".into()).await;
    let (token, _) = app.register_user().await;
    let created = app
        .post_file(
            "/api/estimates",
            "Санузел.xlsx",
            &fixture("sanuzel-novaya-moskva.xlsx"),
            &token,
        )
        .await;
    let id = created.body["id"].as_str().unwrap().to_owned();

    let (shutdown, receiver) = tokio::sync::watch::channel(false);
    server::jobs::spawn(app.pool.clone(), &app.settings, receiver);
    let status = wait_for_final_status(&app.pool, &id).await;
    let _ = shutdown.send(true);

    assert_eq!(status.as_deref(), Some("parsed"), "смета не разобралась");
}

/// Дождаться, пока фоновый цикл доведёт смету до конечного состояния.
/// Ждём с потолком: тест обязан падать, а не висеть.
#[allow(
    clippy::unwrap_used,
    reason = "хелпер теста: паника валит тест — это и есть отчёт об ошибке"
)]
async fn wait_for_final_status(pool: &sqlx::PgPool, id: &str) -> Option<String> {
    for _ in 0..100 {
        let status: String = sqlx::query_scalar("SELECT status FROM estimates WHERE id = $1::uuid")
            .bind(id)
            .fetch_one(pool)
            .await
            .unwrap();
        if status == "parsed" || status == "failed" {
            return Some(status);
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    None
}

#[tokio::test]
async fn promote_admin_changes_role() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;

    assert!(
        server::users::set_role_by_email(&app.pool, &email, server::users::Role::Admin)
            .await
            .unwrap()
    );
    let user = server::users::find_by_email(&app.pool, &email)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        server::users::Role::parse(&user.role),
        server::users::Role::Admin
    );

    // несуществующий адрес — не ошибка выполнения, а честное «не нашёл»
    assert!(
        !server::users::set_role_by_email(
            &app.pool,
            "no-such@test.local",
            server::users::Role::Admin
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn failed_email_can_be_returned_to_queue() {
    let app = spawn_app().await;
    server::core::mailer::send(&app.pool, "to@example.com", "Тема", "Текст")
        .await
        .unwrap();

    // «сжигаем» попытки — воркер такое письмо больше не берёт
    let broken = AsyncStubTransport::new_error();
    for _ in 0..6 {
        server::jobs::deliver_outbox(&app.pool, &broken, &test_sender())
            .await
            .unwrap();
    }
    let failed = server::jobs::failed_emails(&app.pool).await.unwrap();
    assert_eq!(failed.len(), 1);
    let id = failed[0].id;

    assert!(server::jobs::retry_email(&app.pool, id).await.unwrap());
    assert!(
        server::jobs::failed_emails(&app.pool)
            .await
            .unwrap()
            .is_empty(),
        "после retry письмо снова в очереди"
    );
    // и действительно уходит, когда почта заработала
    let smtp = AsyncStubTransport::new_ok();
    let delivered = server::jobs::deliver_outbox(&app.pool, &smtp, &test_sender())
        .await
        .unwrap();
    assert_eq!(delivered, 1);

    assert!(
        !server::jobs::retry_email(&app.pool, id).await.unwrap(),
        "отправленное письмо повторно в очередь не возвращается"
    );
}

#[tokio::test]
async fn cleanup_removes_expired_and_used_tokens() {
    let app = spawn_app().await;
    let (_, email) = app.register_user().await;
    let user_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(&email)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO password_reset_tokens (token_hash, user_id, expires_at)
         VALUES ($1, $2, now() - interval '1 hour')",
    )
    .bind(b"expired".as_slice())
    .bind(user_id)
    .execute(&app.pool)
    .await
    .unwrap();

    server::auth::cleanup_expired_tokens(&app.pool)
        .await
        .unwrap();

    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM password_reset_tokens")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(left, 0);
}
