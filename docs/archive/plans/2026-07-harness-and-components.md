# План: harness качества и базовые компоненты шаблона

Статус: ВЫПОЛНЕНО (2026-07-24) — см. записи в `docs/journal.md`.
Отклонения от плана зафиксированы в конце документа.
Источник: внешнее ревью предложений (июль 2026) + сверка каждого пункта
с реальным кодом репозитория.
Задачи мелкие и независимые, где возможно; каждая заканчивается
зелёным `make verify` и записью в журнал.

## Что уже есть (не делать повторно)

Сверка показала: часть предложений внешнего ревью уже реализована.

- `make check` / `test` / `e2e` / `verify` — это и есть предлагаемое
  разбиение verify-fast / verify-tests / verify-e2e.
- Rate limiter уже берёт клиентский IP из последнего значения
  `X-Forwarded-For` (дописанного нашим Caddy), с фолбэком на адрес
  соединения (`core/rate_limit.rs`).
- Refresh-cookie уже `SameSite=Lax` с узким `Path=/api/auth`,
  `Secure` — флагом `COOKIE_SECURE` (prod-compose его задаёт).
- Graceful shutdown HTTP уже есть (`with_graceful_shutdown` в bin/api).
- Контракт API, i18n-полнота, лимит размера файлов — уже проверяются.

## Дыры, подтверждённые ревизией кода

1. Линтера фронтенда нет вообще (ни ESLint, ни Biome) — только tsc.
2. `JWT_SECRET` читается лениво при первом запросе: прод стартует без
   секрета и падает на первом логине, а не на старте (`auth/jwt.rs`).
3. `/api/health` — статичное `"ok"` без проверки БД; у контейнера `app`
   в `compose.prod.yaml` нет healthcheck, Caddy стартует не дожидаясь.
4. Dockerfile ни разу не собирается в CI: сломанный образ узнаем при деплое.
5. `X-Forwarded-For` доверяется и без прокси: при прямом деплое без Caddy
   rate limit обходится подделкой заголовка.
6. Нет security-сканов (секреты, уязвимые зависимости), GitHub Actions
   закреплены тегами, не SHA.
7. Ошибка валидации — одна на форму, без привязки к полю.
8. `items` не имеет владельца — нет образца row-level authorization.

## Решение: Biome вместо ESLint

Да, и выбор проще, чем в ревью: ESLint в проекте отсутствует, а
typescript-eslint несовместим с TypeScript 7 (заявленная поддержка
`<6.1`), тогда как проект уже на `typescript ~7.0.2`. Реальный выбор —
«Biome или ничего». Biome не зависит от компилятора TS (свой парсер),
даёт formatter + linter + React/a11y-правила одним инструментом.

Ограничения: версию закрепляем точно (без `^`); `biome ci` по всему
коду в CI — это и есть compatibility smoke при обновлениях TS/Biome;
типы проверяет только `tsc --noEmit`. К ESLint возвращаемся только
если появится конкретное typed-правило, которого нет в Biome, и
typescript-eslint начнёт поддерживать TS7.

---

## Этап 1 — Harness: статические проверки (первым: защищает всё дальнейшее)

### H1. Biome как форматтер и линтер фронта (M)

- `@biomejs/biome` точной версией в `web/devDependencies`; `biome.json`:
  formatter (2 пробела, ширина 100), linter recommended + domains
  react/test, `noJsxLiterals` для видимого текста; исключить
  `src/api/schema.d.ts` и `src/components/ui/` (как в check-file-size).
- Прогнать по всему коду, починить найденное (или точечно отключить
  с причиной). `pnpm exec biome ci .` — в `make check` и в CI-джоб web.
- Приёмка: текущий код проходит; фикстура с хардкодным русским текстом
  в JSX и с нарушением rules-of-hooks ломает `make check`.

### H2. Clippy: выборочная lint-таблица workspace (M)

- В `Cargo.toml` workspace: `unsafe_code = "forbid"`, deny для
  `dbg_macro`, `todo`, `unimplemented`, `unwrap_used`, `expect_used`,
  `panic`, `print_stdout`, `print_stderr`,
  `allow_attributes_without_reason`, `unused_async`, `wildcard_imports`.
  В `server/Cargo.toml` — `[lints] workspace = true`.
- `clippy.toml`: `allow-unwrap-in-tests`, `allow-expect-in-tests`,
  `allow-panic-in-tests` = true.
- Починить ~13 существующих `unwrap/expect` вне тестов (в т.ч. H2
  пересекается с B2: `JWT_SECRET.expect` уйдёт в конфиг). Необходимые
  оставить локальным `#[allow(..., reason = "...")]`.
- Целиком `pedantic`/`restriction` не включать (осознанно).
- Приёмка: `make check` зелёный; `unwrap()` в новом доменном коде
  ломает clippy.

### H3. Скрипт policy подавлений (S)

- `scripts/check-suppressions.sh` (по образцу check-file-size): запрет
  `@ts-ignore` (только `@ts-expect-error` с описанием), `test.only`,
  `test.skip` без причины, `#![allow(...)]` на уровне crate,
  `allow(clippy::all)`, `biome-ignore` без объяснения.
- Подключить в `make check` и CI.
- Приёмка: фикстурные нарушения ломают `make check` (проверить руками
  при написании, фикстуры в репо не хранить).

### H4. Hook: расширить file-aware проверки (S)

- В `scripts/hook-check.sh` для изменённого файла добавить:
  `.ts/.tsx` — `biome check <file>` перед tsc; любой файл —
  check-suppressions и лимит строк только для этого файла.
- Не добавлять: clippy, полный vitest, testcontainers — hook остаётся
  быстрым (< 10 с на тёплом кэше).
- Приёмка: правка файла с `@ts-ignore` возвращает ошибку агенту сразу.

## Этап 2 — CI: supply chain и policy

### C1. cargo-deny + cargo-machete (M)

- `deny.toml`: advisories (deny), licenses (allowlist: MIT, Apache-2.0,
  BSD, ISC, Unicode и что реально в графе), bans
  (`multiple-versions = "warn"`, `wildcards = "deny"`), sources
  (unknown registry/git = deny). Исключения — только с reason.
- `cargo-deny` — только CI (нужна сеть; `make verify` остаётся офлайн).
- `cargo-machete` — в `make check` и CI; ложные срабатывания — в
  `package.metadata.cargo-machete.ignored` с комментарием.
- Отдельный `cargo-audit` не ставим — дублирует advisories.
- Приёмка: policy-джоб красный при wildcard-зависимости в фикстурной
  ветке; `make check` ловит неиспользуемый crate.

### C2. Policy-джоб CI (M)

- Новый джоб `policy` (timeout 5 мин): `cargo deny check`, Gitleaks по
  PR-диапазону (бинарём, не официальным action — тому нужна лицензия
  для организаций), OSV-Scanner по `web/pnpm-lock.yaml`, `actionlint`,
  `renovate-config-validator --strict`.
- Все сторонние actions во всех джобах закрепить полным commit SHA
  (Renovate обновляет их отдельными PR).
- Приёмка: фикстурный «секрет» в diff и незакреплённый action ломают джоб.

### C3. Флаги стабильности тестов (S)

- Playwright: `--forbid-only` всегда; в CI `retries: 1` +
  `failOnFlakyTests: true` (восстановившийся тест = ошибка джоба,
  но с двумя trace для диагностики).
- Vitest остаётся `retry: 0`.
- cargo-nextest НЕ вводим: новых флаков нет, инструмент ради
  retry-классификации преждевременен. Вернуться при первом реальном флаке.
- Приёмка: `test.only` в e2e ломает прогон.

### C4. Image-smoke джоб (M, после B1)

- CI собирает Dockerfile, поднимает образ с Postgres (services),
  ждёт `/api/health/ready`, дёргает SPA (`/`) и один API-маршрут.
- Приёмка: сломанный COPY статики или падение миграций на старте
  делает джоб красным.

### C5. CodeQL — условное решение (S)

- Если репозиторий публичный (или куплен GitHub Code Security) —
  включить матрицу rust + javascript-typescript, PR + weekly.
  Для приватного без подписки — недоступен: зафиксировать в журнале
  и не городить замену (Semgrep не добавляем).

## Этап 3 — Инварианты рантайма

### B1. Health / readiness / version + healthcheck прод-приложения (M)

- `/api/health/live` — процесс отвечает; `/api/health/ready` —
  `SELECT 1` с таймаутом ~2 с; `/api/version` — версия и commit SHA
  (build-arg в Dockerfile). Существующий `/api/health` — оставить
  алиасом live либо удалить вместе с обновлением compose.
- `compose.prod.yaml`: healthcheck у `app` на ready (проверить, что в
  образе есть чем дёргать HTTP — иначе подкоманда бинаря `healthcheck`),
  `caddy: depends_on: condition: service_healthy`.
- Graceful: воркер `core/jobs.rs` получает сигнал завершения и не
  берёт новую пачку писем после начала shutdown.
- SMTP/OAuth в readiness НЕ проверяем — их авария не должна
  перезапускать приложение.
- Приёмка: интеграционный тест ready (200 с БД); smoke в C4.

### B2. Типизированная конфигурация `Settings::from_env()` (M)

- Один модуль `core/config.rs`: все ~14 разрозненных `env::var` читаются
  и валидируются один раз до старта HTTP; секретные поля не попадают
  в `Debug`/логи. `JWT_SECRET` обязателен на старте (чинит дыру №2).
- `bin/api` и `bin/bot` получают `Settings`, прямые `env::var` в
  доменах запрещаются (grep-строчка в check-suppressions).
- `/api/meta` НЕ делаем, пока фронту нечего оттуда читать.
- Приёмка: старт без `JWT_SECRET` падает сразу с понятной ошибкой;
  тест на валидацию конфигурации.

### B3. HTTP-hardening (M)

- Timeout-слой на API (~15 с), явный body limit (1 MiB по умолчанию,
  меньше на auth-ручки), `Origin`-проверка на маршрутах, меняющих
  cookie-состояние.
- Доверие `X-Forwarded-For` — только при `TRUST_PROXY=true`
  (prod-compose задаёт; чинит дыру №5).
- Caddyfile: CSP, `X-Content-Type-Options`, `Referrer-Policy`,
  `Permissions-Policy`; immutable-кэш для хешированных assets,
  no-cache для `index.html`.
- Приёмка: тесты на 413/408/запрещённый Origin; заголовки видны в C4.

### B4. Полевые ошибки валидации (M)

- Расширить текущий формат совместимо:
  `{"error": {code, message, fields?: [{field, code, message}]}}`;
  `ApiError::validation_fields(...)`; схема в OpenAPI.
- Фронт: `FieldError` + хелпер разбора полевых ошибок; применить в
  формах регистрации/логина. Сервер — единственный авторитет,
  Zod-дубликат валидации не заводим.
- Приёмка: слишком короткий пароль подсвечивает именно поле пароля
  (component-тест + e2e).

## Этап 4 — Продуктовые компоненты шаблона

### P1. Владение ресурсом на примере items (M)

- `owner_user_id NOT NULL REFERENCES users` (таблица образцовая — в
  миграции допустимо удалить существующие строки с комментарием).
- Доменные функции принимают владельца; SELECT/DELETE всегда с
  ограничением по нему; чужая и несуществующая запись — одинаковый 404.
- Тесты: list/delete между двумя пользователями; универсальный ACL и
  permissions-таблицы не делаем.
- Приёмка: интеграционный тест «пользователь B не видит и не удаляет
  запись пользователя A».

### P2. Self-service аккаунта (L — дробить на 4 задачи)

- P2a: `PATCH /api/me` — имя, локаль.
- P2b: смена пароля (проверка текущего, отзыв всех refresh-сессий).
- P2c: смена email — подтверждение нового адреса (механика one-time
  токенов уже есть), уведомление на старый.
- P2d: список сессий (публичный id, `created_at`, `last_seen_at`,
  краткое описание клиента — без хранения сырого UA/IP бессрочно),
  отзыв одной/всех. Здесь же первый потребитель компонента
  `<DateTime>` (Intl.DateTimeFormat, RFC 3339/UTC с бэка) — отдельной
  задачей его не делаем.
- Приёмка: каждый под-пункт — свой use case-раздел, тесты и e2e.

### P3. Нейтральный AppShell и состояния страниц (M)

- `AppShell`, `PageHeader`, `EmptyState`, `QueryError`,
  `PendingButton`, `RequireAuth`/`RequireAdmin` (только UX-редирект,
  авторизация остаётся на бэке). `FormError` из auth — в общие.
- Существующие страницы перевести на них; в toolbox.md — новые строки.
- Menu builder, breadcrumbs, таблицы, скелетоны — не делаем до
  реального use case.
- Приёмка: ItemsPage/SettingsPage используют общий shell; component-тест
  RequireAuth-редиректа.

### P4. Минимальный операторский CLI (S)

- Отдельный bin в том же crate (`server/src/bin/ops.rs`), разбор
  аргументов через `std::env::args` — без clap ради трёх команд:
  `promote-admin <email>`, `outbox list-failed`, `outbox retry <id>`
  (+ `config check` после B2). Те же доменные функции, нормальные
  exit codes. Проверить, что Dockerfile кладёт бинарь в образ.
- Приёмка: юнит/интеграционный тест на promote-admin и retry.

## Осознанно не делаем (подтверждённый анти-список)

Организации/multitenancy, RBAC-движок, soft delete везде, generic
audit log, query DSL, webhooks/API keys, центр уведомлений,
универсальная idempotency, CAPTCHA, конструкторы форм/EAV.

Плюс отклонено из предложений ревью: ESLint+Prettier (см. решение),
cargo-nextest (нет флаков), coverage-гейты и плановые coverage/mutants/
flake-stress джобы (на размере шаблона — шум; вернуться при первом
survivor-инциденте или реальном флаке), GritQL-плагин Biome (начать со
встроенного `noJsxLiterals`, плагин — если его не хватит), `/api/meta`,
общий reqwest-клиент (один потребитель — OAuth), backup/restore smoke
в CI (восстановление задокументировано; проверять руками при правке
скрипта — долг в журнале), обязательный pre-commit framework.

## Порядок и зависимости

1. Этап 1 (H1–H4) — первым: дальнейшие правки уже под защитой.
2. Этап 2: C1–C3 сразу после; C4 — после B1; C5 — когда решится
   вопрос видимости репозитория.
3. Этап 3 (B1–B4) — B2 удобно делать вместе с хвостом H2.
4. Этап 4 (P1–P4) — в любом порядке, P2 дробить обязательно.

Бюджеты не меняются: `make verify` локально < 5 мин; каждая задача —
отдельная запись в журнале, заметные (P1, P2, B4) — с use case.

---

## Что сделано иначе, чем в плане (и почему)

- **C5 (CodeQL)** — репозиторий приватный, GitHub Code Security не куплен:
  CodeQL недоступен. Замену (Semgrep и т.п.) не городим, как и договаривались.
- **P1** — удаление чужой записи оставлено администратору: иначе роль
  `admin` и команда `ops promote-admin` не гасили бы ни одного маршрута,
  то есть проверка роли осталась бы без единого потребителя. Для обычного
  пользователя правило плана соблюдено: чужая запись = 404, как и
  несуществующая.
- **P2a** — маршрут `PATCH /api/users/me`, а не `/api/me`: рядом уже жил
  `GET /api/users/me`, две разные формы одного адреса путали бы больше,
  чем экономили.
- **P2d** — список сессий живёт под `/api/auth/sessions`: refresh-cookie
  ограничена путём `/api/auth`, без неё нельзя понять, какая сессия текущая.
- **B3** — тест на 408 не написан: свой код таймаута мы не пишем, это
  трёхстрочная настройка слоя tower-http, и тест проверял бы чужой таймер.
  Тесты на 413 и запрещённый Origin есть.
- **B4** — вместо `ApiError::validation_fields(...)` сделан `.field("имя")`:
  домен всегда возвращает одну ошибку за раз. Формат ответа остался как в
  плане (`fields` — массив), поэтому расширение до нескольких полей не
  ломает контракт.
- **H2** — в тест-крейте `server/tests` разрешены `unwrap/expect` целиком:
  `clippy.toml` снимает запрет только внутри самих `#[test]`-функций, а у
  нас половина работы во вспомогательных функциях `common.rs`. Скрипт
  подавлений проверяет crate-level `allow` только в `server/src`.
