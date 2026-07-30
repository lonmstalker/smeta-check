-- Самообслуживание аккаунта: профиль, смена почты и видимые сессии.

-- Имя и язык — то, что пользователь меняет сам в настройках.
ALTER TABLE users ADD COLUMN display_name text;
ALTER TABLE users ADD COLUMN locale text;

-- Смена почты подтверждается письмом на НОВЫЙ адрес (тот же паттерн
-- одноразовых токенов, что у сброса пароля: в базе только хеш).
CREATE TABLE email_change_tokens (
    token_hash bytea PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    new_email  text NOT NULL,
    expires_at timestamptz NOT NULL,
    used_at    timestamptz
);

-- Список сессий. Refresh-токен при каждом обновлении ротируется — появляется
-- новая строка, — поэтому у сессии есть сквозной session_id и время начала:
-- иначе пользователь видел бы новую «сессию» каждые пятнадцать минут.
-- created_at живой строки = когда сессией пользовались в последний раз.
--
-- Сырые User-Agent и IP не храним: они точнее, чем нужно, и живут вечно.
-- Достаточно короткого описания вида «Chrome, macOS».
ALTER TABLE refresh_tokens ADD COLUMN session_id uuid NOT NULL DEFAULT gen_random_uuid();
ALTER TABLE refresh_tokens ADD COLUMN started_at timestamptz NOT NULL DEFAULT now();
ALTER TABLE refresh_tokens ADD COLUMN client text;
CREATE INDEX refresh_tokens_session_idx ON refresh_tokens (user_id, session_id);
