#!/bin/sh
# Большой файл съедает контекст человека и нейронки. Цель — до 300 строк,
# жёсткий потолок — 400: превысил, дроби по смыслу (подмодули домена,
# отдельные страницы/компоненты; у доков — вынос старого в docs/archive/).
# Исключения: генерённое, вендорное и архив документации.
limit=400
fail=0
cd "$(dirname "$0")/.."
# --others: новый файл проверяется до первого git add, а не после
for f in $(git ls-files --cached --others --exclude-standard \
    '*.rs' '*.ts' '*.tsx' '*.md' '*.mjs' '*.sql' '*.css' '*.ftl' \
    | grep -v 'schema.d.ts' \
    | grep -v 'web/src/components/ui/' \
    | grep -v 'docs/archive/'); do
  [ -f "$f" ] || continue # удалённый файл ещё числится в индексе
  lines=$(wc -l < "$f")
  if [ "$lines" -gt "$limit" ]; then
    echo "СЛИШКОМ БОЛЬШОЙ ФАЙЛ: $f ($lines строк > $limit) — раздели или заархивируй" >&2
    fail=1
  fi
done
exit $fail
