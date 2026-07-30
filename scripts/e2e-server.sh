#!/bin/sh
# Полный стек для e2e: Postgres (compose) -> сборка фронта и api -> api на 8081.
# Запускается и гасится Playwright'ом (webServer); exec ниже нужен, чтобы
# Playwright убивал сам процесс api, а не только оболочку.
set -e
cd "$(dirname "$0")/.."

docker compose up -d --wait db
# отдельная БД e2e, чтобы не гадить в dev-данные
docker compose exec -T db psql -U postgres -tc "SELECT 1 FROM pg_database WHERE datname='e2e'" | grep -q 1 \
  || docker compose exec -T db createdb -U postgres e2e

(cd web && pnpm build)
cargo build --bin api

DATABASE_URL=postgres://postgres:dev@localhost:5432/e2e \
JWT_SECRET=e2e-secret-not-for-prod \
METRICS_ADDR=127.0.0.1:0 \
RATE_LIMIT_AUTH_RPM=0 \
PUBLIC_URL=http://localhost:8081 \
PORT=8081 exec ./target/debug/api
