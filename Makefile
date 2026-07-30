# Единая точка входа. Перед сдачей любой задачи: make verify
.PHONY: dev dev-mail check test e2e visual verify gen-api mutants

# Быстрый путь тестов: если dev-БД поднята (make dev) — тестовые базы
# создаются на ней за миллисекунды и контейнеры не поднимаются вовсе
TEST_PG_URL := $(shell docker compose ps -q --status running db 2>/dev/null | grep -q . && echo postgres://postgres:dev@localhost:5432/postgres)

# БД для локальной разработки (tmpfs, данные эфемерны)
dev:
	docker compose up -d --wait db

# То же + локальный SMTP (Mailpit): письма видны на http://localhost:8025.
# api запускать с SMTP_URL=smtp://localhost:1025
dev-mail:
	docker compose --profile mail up -d --wait db mailpit

# Быстрые статические проверки
check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cd web && pnpm exec biome ci .
	cd web && pnpm exec tsc --noEmit
	sh scripts/check-file-size.sh
	sh scripts/check-suppressions.sh
	cargo machete

# Юнит- и интеграционные тесты (нужен запущенный docker)
test:
	TEST_PG_URL=$(TEST_PG_URL) cargo test
	cd web && pnpm test

# Сквозные тесты: браузер -> собранный фронт -> axum -> Postgres
e2e:
	cd web && pnpm test:e2e

# Визуальные страницы локально: прогон без сравнения эталонов
# (эталоны рендерит и сверяет только CI-джоб visual — другие пиксели)
visual:
	cd web && pnpm build && pnpm exec playwright test -c playwright.visual.config.ts

# Полная проверка перед сдачей задачи
verify: check test e2e

# Мутационные тесты: после ПОЛНОЙ реализации фичи, не в цикле разработки.
# Дифф против master; весь проект: sh scripts/mutants.sh --all
mutants:
	sh scripts/mutants.sh

# Перегенерация контракта API: openapi.json + typescript-типы фронта.
# Запускать после любого изменения HTTP-слоя (тест contract напомнит).
gen-api:
	UPDATE_OPENAPI=1 cargo test --test api contract
	cd web && pnpm gen:api
