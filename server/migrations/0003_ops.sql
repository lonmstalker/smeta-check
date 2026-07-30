-- Подтверждение email: отметка на пользователе + одноразовые токены
-- (тот же паттерн, что password_reset_tokens: храним только хеш).
ALTER TABLE users ADD COLUMN email_verified_at timestamptz;

CREATE TABLE email_verification_tokens (
    token_hash bytea PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL,
    used_at    timestamptz
);

-- outbox_emails становится очередью доставки: sent_at IS NULL — письмо ещё
-- не ушло по SMTP; attempts ограничивает повторы «ядовитого» письма.
ALTER TABLE outbox_emails ADD COLUMN sent_at timestamptz;
ALTER TABLE outbox_emails ADD COLUMN attempts int NOT NULL DEFAULT 0;
