#!/bin/sh
# PostToolUse-хук Claude Code: после каждой правки файла — мгновенная проверка.
# Ошибки уходят в stderr с exit 2, и агент чинит их сразу, не в конце.
# Хук обязан быть быстрым (< 10 с на тёплом кэше): здесь только то, что
# смотрит на изменённый файл. Clippy, тесты и контейнеры — в `make verify`.
file=$(python3 -c "import json,sys; print(json.load(sys.stdin).get('tool_input',{}).get('file_path',''))" 2>/dev/null)
cd "$(dirname "$0")/.." || exit 0
[ -n "$file" ] || exit 0
root=$(pwd)
# правила подавлений различают server/src и остальное — нужен путь от корня
rel=${file#"$root"/}
[ -f "$rel" ] || exit 0

fail() {
  printf '%s\n' "$1" | tail -60 >&2
  exit 2
}

# Метка «код менялся» для Stop-хука (hook-stop.sh): сдача без записи
# в журнал получает напоминание. Запись в журнал снимает метку.
case "$rel" in
  docs/journal.md) rm -f .git/claude-pending-journal ;;
  *.rs|*.ts|*.tsx|*.sql|*.css|*.ftl|*.mjs) touch .git/claude-pending-journal ;;
esac

case "$rel" in
  *.rs|*.ts|*.tsx|*.sh|*.yml|*.yaml)
    out=$(sh scripts/check-suppressions.sh "$rel" 2>&1) || fail "$out" ;;
esac
case "$rel" in
  # исключения — те же, что в check-file-size.sh
  docs/archive/*|*schema.d.ts|web/src/components/ui/*) ;;
  *.rs|*.ts|*.tsx|*.md|*.sql|*.css|*.ftl|*.mjs)
    lines=$(wc -l < "$rel")
    if [ "$lines" -gt 400 ]; then
      fail "СЛИШКОМ БОЛЬШОЙ ФАЙЛ: $rel ($lines строк > 400) — раздели по смыслу"
    fi ;;
esac

case "$rel" in
  *.rs)
    out=$(cargo check --workspace --all-targets 2>&1) || fail "$out" ;;
  *.ts|*.tsx)
    # вендорное и генерённое biome игнорирует и падает с «0 files» — не зовём
    case "$rel" in
      web/src/components/ui/*|*schema.d.ts) ;;
      *) out=$(cd web && pnpm exec biome check "$root/$rel" --colors=off 2>&1) || fail "$out" ;;
    esac
    out=$(cd web && pnpm exec tsc --noEmit 2>&1) || fail "$out" ;;
  web/*.mjs)
    # mjs вне tsconfig, но biome его проверяет (motion.mjs однажды проехал мимо)
    out=$(cd web && pnpm exec biome check "$root/$rel" --colors=off 2>&1) || fail "$out" ;;
esac
exit 0
