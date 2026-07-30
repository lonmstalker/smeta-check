# Индекс общих компонентов и хелперов

Перед тем как писать функцию/компонент/хелпер — проверь этот список.
Добавил что-то общее — допиши строку здесь (в том же изменении).
Правило дублирования — в AGENTS.md.

## Бэкенд: `server/src/core/` (инфраструктура, не знает о доменах)

| Модуль | Что даёт |
|--------|----------|
| `config.rs` | `Settings::from_env()` — ВСЯ конфигурация одним объектом (единственное место, где читается окружение); `Settings::for_tests()`; тип `Secret` (не печатается в логах); `OauthProvider` |
| `db.rs` | `connect(url)` — пул; `MIGRATIONS` — миграции sqlx |
| `health.rs` | `router()` — `/api/health/{live,ready}` и `/api/version` |
| `error.rs` | `ApiError` — единая ошибка API с ключом локализации: `validation/unauthorized/forbidden/not_found/conflict/too_large/too_many_requests`, `.arg(name, value)` для подстановок, `.field("password")` — привязать к полю формы; `From<anyhow>` → 500 без деталей наружу |
| `i18n.rs` | `translate(lang, key, args)`, `current_lang()`, `lang_middleware` (язык из Accept-Language), `message_keys(lang)` — для тестов полноты словарей |
| `telemetry.rs` | `init_tracing()`, `init_metrics()`, `track_http` (лог+метрики каждого запроса), `request_id_layer()`, `REQUEST_ID_HEADER` |
| `mailer.rs` | `send(pool, to, subject, body)` — ЕДИНСТВЕННАЯ точка отправки писем (кладёт в outbox-очередь) |
| `rate_limit.rs` | `limit_per_ip(router, rpm, trust_proxy)` — лимит запросов по IP на группу маршрутов |
| `storage.rs` | `save(dir, name, bytes)` / `read(dir, name)` — файлы пользователей на диске (`Settings::files_dir`); имя собирает домен из id записи |
| `time.rs` | `rfc3339(OffsetDateTime)` — время для ответов API (UTC, форматирует браузер) |

## Бэкенд: `server/src/jobs.rs` (фоновый воркер приложения)

`spawn(pool, settings, shutdown)` — один цикл в процессе api: отдаёт очередь
писем (`deliver_outbox(pool, smtp, from)`), раз в час зовёт чистку токенов
домена auth и `cleanup_sent_emails(pool)`. Для разбора руками —
`failed_emails(pool)` и `retry_email(pool, id)` (см. `bin/ops.rs`). Живёт вне
`core`, потому что знает о доменах; новую периодическую задачу — сюда.

## Бэкенд: переиспользуемое из доменов

| Где | Что даёт |
|-----|----------|
| `users/http.rs` | extractors `CurrentUser` (только вошедшие) и `AdminUser` (только админ) — добавь параметром хендлера |
| `auth/jwt.rs` | `sign_access/verify_access`, `sign_pending_2fa/verify_pending_2fa`, TTL-константы |
| `auth/password.rs` | `hash/verify`, `MIN_PASSWORD_LEN` (связан со спекой тестом contract) |
| `auth/mod.rs` | `hash_token/new_raw_token` (crate-private) — образец одноразовых токенов: reset, verify-email и смена почты уже так сделаны; `normalize_email`, `validate_password(pw, field)`; `cleanup_expired_tokens(pool)` — чистка своих таблиц |
| `auth/oauth.rs` | `require_provider(settings, name)`, `find_or_create_user(pool, cfg, id, email)` — вход через провайдера (флаг `trust_email`, см. docs/oauth.md) |
| `auth/sessions.rs` | `issue/refresh/logout`, `revoke_all/revoke/revoke_others`, `list` (что видит пользователь), `describe_client(ua)` — «Chrome, macOS» вместо сырого User-Agent |
| `auth/account.rs` | `change_password`, `request_email_change/confirm_email_change` — обе операции требуют текущий пароль |
| `users/mod.rs` | `update_profile` (NULL = не менять), `set_email`, `set_role_by_email` |
| `estimates/mod.rs` | сметы: `create/list/get/count_of` (все с владельцем), `extension_of` (только xlsx/xls), `stored_name(id, ext)`, `clean_file_name`, потолки `MAX_FILE_BYTES`/`MAX_PER_USER` |
| `items/mod.rs` | образец row-level authorization: все функции принимают владельца и фильтруют по нему прямо в SQL |
| `lib.rs` | `AppState` (пул + `Arc<Settings>`); хендлеру нужен пул — `State<PgPool>`, нужна настройка — `State<Arc<Settings>>` |

## Тест-инфраструктура бэкенда: `server/tests/api/common.rs`

`spawn_app()` — приложение на чистой БД одной строкой; `TestApp`:
`get/get_auth/post/post_auth/patch_auth/delete_auth/request` (произвольные
заголовки), `register_user()`, `promote_to_admin(email)`,
`refresh_token_of(res)`, `last_email_to(recipient)` (читает outbox);
`post_file(path, file_name, bytes, token)` — загрузка файла (тело multipart
собирается внутри), `files_dir` — каталог файлов этого теста (стирается сам);
`fixture(name)` — настоящая смета из `tests/fixtures/estimates`;
константа `PASSWORD`. Нужны другие настройки — `spawn_app_with(|s| ...)`:
окружение процесса тесты не трогают.

## Фронт: `web/src/lib/` и обвязка

| Файл | Что даёт |
|------|----------|
| `api/client.ts` | `api.get/post/patch/delete` (авто-refresh при 401), `setAccessToken`, `setOnSessionExpired(cb)` (сессия умерла — интерфейсу пора выйти), класс `ApiError` (+ `fields`), `fieldError(error, field)`, типы API из `schema.d.ts` (`User`, `Item`, `SessionInfo`, …) — руками типы не писать |
| `auth/AuthContext.tsx` | `useAuth()`: `user`, `ready`, `register`, `login` (вернёт pending-токен при 2FA), `verify2fa`, `logout`, `refreshUser` |
| `lib/utils.ts` | `cn(...)` — слияние tailwind-классов |
| `lib/logger.ts` | `log.info/warn/error` (warn/error улетают на бэк), `installGlobalErrorLogging()` |
| `lib/theme.ts` | `applyStoredTheme()`, `toggleTheme()` — тёмная тема |
| `lib/i18n.ts` | инициализация i18next; словари `src/locales/{ru,en}.json` |
| `scripts/motion.mjs` | `pnpm motion <url> [селектор]` — проверка анимаций и реакции на клик: JSON-сводка (анимации, DOM-изменения, плавность, запросы) + 3 кадра; `--self-test` для самопроверки |

## Фронт: компоненты

| Файл | Что даёт |
|------|----------|
| `components/ui/*` | вендорные shadcn: `Button`, `Card`, `Input` — новые брать через `pnpm dlx shadcn add` |
| `components/ErrorBoundary.tsx` | ловит ошибки рендера: заглушка вместо белого экрана + лог на бэк |
| `components/AppShell.tsx` | `AppShell` (шапка), `Page` (колонка контента), `PageHeader` — общий каркас всех обычных страниц |
| `components/form.tsx` | `FormError` (ошибка про форму целиком), `FieldError` (про поле), `invalid(error, field)` — подсветка поля |
| `components/states.tsx` | `EmptyState`, `QueryError` (с кнопкой «Повторить»), `PendingButton` (сама блокируется на время запроса) |
| `components/guards.tsx` | `RequireAuth`, `RequireAdmin` — только UX-редирект, права проверяет бэкенд |
| `components/DateTime.tsx` | `<DateTime value={rfc3339} />` — время с бэка в местном формате браузера |
| `pages/auth/AuthLayout.tsx` | `AuthCard` — обёртка форм входа |
| `sonner` | тосты: `<Toaster />` уже в App, зови `toast.info/error(...)` |

## Тест-инфраструктура фронта

| Файл | Что даёт |
|------|----------|
| `test-utils.tsx` | `renderApp(ui, {route})` — рендер на нужном адресе (провайдеры заводит сам `App`, второй комплект не нужен); `mockApi(routes)` — мок сети по путям (ответчик получает `url, init`); пресеты `guestApi(extra)` и `authedApi(extra)` |
| `e2e/fixtures.ts` | фикстура `user` — тест начинается уже вошедшим свежим пользователем |
| `scripts/check-animations.mjs` | статический чек собранного CSS: анимируются только не-layout свойства; исключения — `scripts/animation-allowlist.txt` с причиной; в CI после `pnpm build` |
