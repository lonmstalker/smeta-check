-- У записи появляется владелец. Это образец row-level authorization: домен
-- всегда фильтрует выборку по владельцу, а не проверяет права «сверху» — тогда
-- чужую запись невозможно достать даже по ошибке в новом запросе.
--
-- Таблица items показательная и настоящих данных не хранит, а взять владельца
-- для старых строк неоткуда, поэтому их удаляем.
DELETE FROM items;

ALTER TABLE items
    ADD COLUMN owner_user_id uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE;

-- список всегда «мои записи, новые сверху» — индекс ровно под этот запрос
CREATE INDEX items_owner_idx ON items (owner_user_id, id DESC);
