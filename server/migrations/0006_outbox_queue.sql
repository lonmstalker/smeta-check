-- Очередь писем читается только по неотправленным. Частичный индекс держит
-- выборку дешёвой, даже когда в таблице накопится история отправленного.
CREATE INDEX outbox_emails_pending_idx
    ON outbox_emails (id) WHERE sent_at IS NULL;
