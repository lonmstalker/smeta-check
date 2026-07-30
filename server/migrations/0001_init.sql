CREATE TABLE items (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title      text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
