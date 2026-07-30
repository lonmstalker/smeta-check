# smeta-check

Сервис проверки смет на ремонт квартир: загружаешь смету бригады —
получаешь разбор против рыночных цен своего города, список пропущенных
работ и вопросы бригаде. Подробнее — [docs/product.md](docs/product.md).

Технически: Rust-бэкенд (axum + sqlx + PostgreSQL 18)
и SPA-фронт (Vite 8 + React 19 + Tailwind 4 + shadcn/ui). Из коробки:
учётные записи (JWT, 2FA, восстановление пароля, роли, вход через
VK/Яндекс конфигом), самообслуживание аккаунта (профиль, смена пароля и
почты, список устройств), OpenAPI-контракт с генерацией типов, локализация
ru/en со склонениями, логи с request-id, метрики Prometheus, проверки
готовности, тесты трёх уровней на реальной БД и проверки цепочки поставки
в CI.

Читать по порядку:
[docs/product.md](docs/product.md) — что за продукт;
[docs/architecture.md](docs/architecture.md) — как устроен;
[docs/usecases/](docs/usecases) — сценарии;
[docs/journal.md](docs/journal.md) — история простым русским;
[AGENTS.md](AGENTS.md) — правила для агентов и людей.

## Команды

```sh
cp .env.example .env
make dev      # Postgres для локальной разработки (tmpfs)
cargo run --bin api                # http://localhost:8080
cd web && pnpm install && pnpm dev # http://localhost:5173 (прокси на 8080)

make check    # fmt, clippy, biome, tsc, размер файлов, подавления, зависимости
make test     # cargo test + vitest; с поднятой dev-БД — быстрый путь
make e2e      # браузер -> собранный фронт -> axum -> Postgres
make verify   # всё вместе — перед сдачей задачи
make gen-api  # перегенерация openapi.json + типов фронта
```

## Структура

```
server/            один crate, домены как модули (DDD-lite)
  src/core/        инфраструктура: конфиг, db, ошибки+i18n, логи+метрики,
                   mailer, health/version
  src/jobs.rs      фоновый воркер: очередь писем, чистка токенов
  src/auth/        вход, JWT, 2FA, восстановление, OAuth (docs/oauth.md)
  src/users/       пользователи, роли, extractor'ы CurrentUser/AdminUser
  src/items/       примерная сущность — переименовать в первую настоящую
  src/bin/         api (сервер) и ops (операторские команды на проде)
  tests/api/       интеграционные тесты на реальном Postgres
web/               React SPA
  src/api/         клиент + типы, генерируемые из openapi.json
  src/locales/     словари ru/en (склонения включены)
  e2e/             сквозные сценарии (playwright)
.claude/skills/    скиллы: workflow, new-feature, writing-tests, ux
openapi.json       контракт API (тест не даст ему устареть)
```

## Решения (почему так)

- **Rust + один процесс** (API + статика): самый дешёвый VPS тянет прод.
- **Часовых сборок нет**: локально только dev-инкрементал; release с LTO
  собирают CI и Dockerfile.
- **Контракт не может соврать**: спека генерируется из кода, тесты ловят
  дрифт и на бэке (openapi.json), и на фронте (schema.d.ts).
- **Тесты экономят ресурсы**: с поднятой dev-БД — база на тест за
  миллисекунды без контейнеров; иначе testcontainer на тест с
  гарантированной очисткой; таймауты на всех уровнях — ничего не виснет.
- **Postgres 18, не 19**: 19 на июль 2026 — только beta; беты в прод не берём.
- **Настройки читаются один раз на старте** (`core/config.rs`): неверная
  конфигурация = процесс не поднялся, а не «упало через неделю».
- **Подавить проверку можно только с причиной**: `scripts/check-suppressions.sh`
  ловит `@ts-ignore`, `.only`, отключение линтера на весь файл и т.п.
