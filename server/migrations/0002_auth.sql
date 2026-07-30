CREATE TABLE users (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    email         text NOT NULL UNIQUE,
    -- NULL, если пользователь пришёл только через VK/Яндекс и пароль не задавал
    password_hash text,
    role          text NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'admin')),
    -- NULL = второй фактор выключен
    totp_secret   text,
    created_at    timestamptz NOT NULL DEFAULT now()
);

-- Сессии: refresh-токен храним только хешем; ротация при каждом обновлении
CREATE TABLE refresh_tokens (
    id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash bytea NOT NULL UNIQUE,
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX refresh_tokens_user_idx ON refresh_tokens (user_id);

-- Одноразовые токены восстановления пароля (тоже только хешем)
CREATE TABLE password_reset_tokens (
    token_hash bytea PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    expires_at timestamptz NOT NULL,
    used_at    timestamptz
);

-- Привязки внешних провайдеров входа (VK ID, Яндекс ID, ...)
CREATE TABLE identities (
    provider         text NOT NULL,
    provider_user_id text NOT NULL,
    user_id          uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at       timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, provider_user_id)
);

-- Дев-почтовый ящик: в dev письма складываются сюда вместо SMTP
CREATE TABLE outbox_emails (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    recipient  text NOT NULL,
    subject    text NOT NULL,
    body       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
