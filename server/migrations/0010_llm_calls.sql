-- Учёт вызовов нейросети: сколько токенов потрачено и когда. Только учёт —
-- ни запросов, ни ответов здесь нет: фото пользователя и текст сметы в базу
-- не ложатся, поэтому нечему утекать и нечего чистить, кроме счётчиков.
--
-- Зачем таблица, а не счётчик в памяти: потолок обязан пережить перезапуск
-- процесса, иначе дневной лимит обнуляется каждым деплоем.
CREATE TABLE llm_calls (
    id                bigserial PRIMARY KEY,
    -- какая модель отвечала: при смене модели видно, где кончились токены
    model             text NOT NULL,
    prompt_tokens     integer NOT NULL CHECK (prompt_tokens >= 0),
    completion_tokens integer NOT NULL CHECK (completion_tokens >= 0),
    created_at        timestamptz NOT NULL DEFAULT now()
);

-- единственный запрос к таблице — «сколько потрачено за сегодня»
CREATE INDEX llm_calls_created_idx ON llm_calls (created_at);
