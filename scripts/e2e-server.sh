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

# файлы стенда живут в target: он и так не в git, и чистится вместе со сборкой
rm -rf target/e2e-files

DATABASE_URL=postgres://postgres:dev@localhost:5432/e2e \
JWT_SECRET=e2e-secret-not-for-prod \
METRICS_ADDR=127.0.0.1:0 \
RATE_LIMIT_AUTH_RPM=0 \
RATE_LIMIT_UPLOAD_RPM=0 \
FILES_DIR=target/e2e-files \
WORKER_TICK_SECS=1 \
PUBLIC_URL=http://localhost:8081 \
PORT=8081 exec ./target/debug/api
