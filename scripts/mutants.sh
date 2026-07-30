#!/bin/sh
# Мутационные тесты: ловят тесты, которые ничего не проверяют.
# Запускать ПОСЛЕ полной реализации фичи (make mutants), не в цикле
# разработки: прогон долгий — каждый выживший мутант означает, что код
# можно сломать и ни один тест этого не заметит.
#
# По умолчанию мутируется только дифф против master (то, что сделала
# фича) в обоих стеках: server — cargo-mutants, web — StrykerJS.
# `scripts/mutants.sh --all` — весь проект целиком (например, ночью).
set -eu

cd "$(dirname "$0")/.."

# Щадящий режим для ноутбука: пониженный QoS уводит сборку на
# энергоэффективные ядра — медленнее, зато без турбин и троттлинга.
# ponytail: один рычаг (QoS) вместо тонкой настройки числа потоков.
if command -v taskpolicy >/dev/null 2>&1; then
  CALM="taskpolicy -c utility"
else
  CALM="nice -n 19"
fi

# Без поднятой dev-БД каждый rust-мутант поднимал бы testcontainers
# с нуля — это в разы дольше и горячее. Требуем make dev заранее.
require_db() {
  if ! docker compose ps -q --status running db 2>/dev/null | grep -q .; then
    echo "dev-БД не запущена: сначала make dev (иначе каждый мутант будет поднимать контейнеры)" >&2
    exit 1
  fi
  TEST_PG_URL=postgres://postgres:dev@localhost:5432/postgres
  export TEST_PG_URL
}

if ! command -v cargo-mutants >/dev/null 2>&1; then
  echo "cargo-mutants не установлен. Установка: cargo install cargo-mutants --locked" >&2
  exit 1
fi

STATUS=0

if [ "${1:-}" = "--all" ]; then
  require_db
  $CALM cargo mutants || STATUS=$?
  (cd web && $CALM pnpm exec stryker run) || STATUS=$?
else
  BASE=$(git merge-base master HEAD)
  DIFF_FILE=$(mktemp)
  trap 'rm -f "$DIFF_FILE"' EXIT
  git diff "$BASE" > "$DIFF_FILE"
  if ! [ -s "$DIFF_FILE" ]; then
    echo "Нет изменений против master — нечего мутировать. Весь проект: scripts/mutants.sh --all"
    exit 0
  fi

  # --- server: cargo-mutants сам выберет rust-файлы из диффа
  if grep -q '^+++ .*\.rs$' "$DIFF_FILE"; then
    require_db
    $CALM cargo mutants --in-diff "$DIFF_FILE" || STATUS=$?
  fi

  # --- web: изменённые исходники (без тестов, генерённого и вендорного)
  FRONT=$(git diff --name-only --diff-filter=d "$BASE" -- web/src \
    | grep -E '\.tsx?$' \
    | grep -v -e '\.test\.' -e 'src/api/schema.d.ts' -e 'src/components/ui/' \
    | sed 's|^web/||' | paste -sd, - || true)
  if [ -n "$FRONT" ]; then
    (cd web && $CALM pnpm exec stryker run --mutate "$FRONT") || STATUS=$?
  fi
fi

# cargo-mutants: код 2 — есть выжившие мутанты, перечень дыр в тестах.
if [ "$STATUS" = 2 ] && [ -f mutants.out/missed.txt ]; then
  echo ""
  echo "Выжившие rust-мутанты (код ломается — тесты молчат), mutants.out/missed.txt:"
  cat mutants.out/missed.txt
fi
exit "$STATUS"
