-- Сметы пользователя. Файл лежит на диске (FILES_DIR), в базе — карточка:
-- кто загрузил, как файл назывался у человека, в каком он состоянии.
--
-- id — uuid, а не счётчик: ссылка на смету не должна перебираться по номерам.
-- Состояние разбора живёт здесь же (без универсальной таблицы задач): фоновая
-- задача пока одна, и своя колонка дешевле общей очереди с payload'ами.
CREATE TABLE estimates (
    id                 uuid PRIMARY KEY,
    owner_user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- имя файла у пользователя: показываем его в списке
    file_name          text NOT NULL,
    -- расширение сохранённого файла (оно же имя на диске: <id>.<ext>)
    file_ext           text NOT NULL CHECK (file_ext IN ('xlsx', 'xls')),
    size_bytes         bigint NOT NULL CHECK (size_bytes > 0),
    status             text NOT NULL DEFAULT 'uploaded'
                       CHECK (status IN ('uploaded', 'parsing', 'parsed', 'failed')),
    -- ключ локализации причины отказа; текст подставляется при отдаче
    error_key          text,
    -- сколько раз брали в разбор: после потолка смета больше не берётся
    attempts           integer NOT NULL DEFAULT 0,
    -- когда взяли в работу: зависший разбор возвращается в очередь по времени
    parsing_started_at timestamptz,
    created_at         timestamptz NOT NULL DEFAULT now()
);

-- список всегда «мои сметы, новые сверху» — индекс ровно под этот запрос
CREATE INDEX estimates_owner_idx ON estimates (owner_user_id, created_at DESC);

-- очередь разбора: воркер смотрит только на незаконченные сметы
CREATE INDEX estimates_pending_idx ON estimates (created_at)
    WHERE status IN ('uploaded', 'parsing');
