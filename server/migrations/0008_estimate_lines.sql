-- Строки разобранной сметы. Храним и сырой текст строки, и распознанные
-- поля: нераспознанное не выбрасывается, а показывается пользователю блоком
-- «спросите бригаду, что это» (UC-003.5) — это тоже ценность, а не брак.
--
-- Числа — double precision, а не numeric: мы сравниваем и показываем цены,
-- а не ведём бухгалтерию; точности f64 для этого с запасом.
CREATE TABLE estimate_lines (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    estimate_id uuid NOT NULL REFERENCES estimates (id) ON DELETE CASCADE,
    -- порядок строк как в файле
    position    integer NOT NULL,
    sheet       text NOT NULL,
    -- строка целиком, как её видел человек в Excel
    raw_text    text NOT NULL,
    -- распознанные поля; NULL — эту часть строки понять не удалось
    title       text,
    unit        text,
    quantity    double precision,
    price       double precision,
    total       double precision
);

CREATE INDEX estimate_lines_estimate_idx ON estimate_lines (estimate_id, position);
