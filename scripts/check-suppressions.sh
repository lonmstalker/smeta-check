#!/bin/sh
# Подавление проверки — это долг. Оно допустимо, но обязано быть точечным и
# с причиной словами. Скрипт ловит способы «сделать зелёным, не починив»:
# отключить проверку типов, прогнать один тест вместо всех, заглушить линтер
# целиком, прочитать окружение мимо единственной точки конфигурации.
#
# Принимает список файлов (для хука по одному файлу); без аргументов проверяет
# весь репозиторий.
cd "$(dirname "$0")/.." || exit 1
fail=0

if [ "$#" -gt 0 ]; then
  all=$*
else
  all=$(git ls-files '*.ts' '*.tsx' '*.rs' '*.sh' '*.yml' '*.yaml')
fi
# генерённое и вендорное не наше — правила к нему не применяем
all=$(printf '%s\n' $all | grep -v 'schema.d.ts' | grep -v 'web/src/components/ui/')

# $1 — шаблон grep -E, $2 — объяснение человеку, $3 — набор файлов
forbid() {
  [ -z "$3" ] && return 0
  hits=$(printf '%s\n' $3 | xargs grep -nE "$1" 2>/dev/null)
  [ -z "$hits" ] && return 0
  echo "ПОДАВЛЕНИЕ ПРОВЕРКИ: $2" >&2
  printf '%s\n' "$hits" | sed 's/^/  /' >&2
  fail=1
}

ts=$(printf '%s\n' $all | grep -E '\.tsx?$')
rs=$(printf '%s\n' $all | grep -E '\.rs$')
rs_src=$(printf '%s\n' $rs | grep '^server/src/')

# @ts-ignore глушит ошибку навсегда; @ts-expect-error сам упадёт, когда
# ошибка исчезнет — и подавление уйдёт вместе с ней
forbid '@ts-ignore' \
  'вместо @ts-ignore — @ts-expect-error с описанием, почему так' "$ts"
forbid '@ts-expect-error *$|@ts-expect-error *(--)?>?$' \
  '@ts-expect-error без описания причины' "$ts"

# .only оставляет в CI один тест вместо всех и выглядит как зелёный прогон
forbid '(test|it|describe)\.only\(' \
  '.only прогоняет один тест вместо всех — убери перед сдачей' "$ts"
# пропуск теста допустим, но рядом должно быть написано почему
forbid '(test|it|describe)\.skip\(.*[^/]$' \
  'пропуск теста без причины — допиши комментарий // почему в той же строке' "$ts"

# biome сам разрешает подавление, но нам нужна причина после двоеточия
forbid 'biome-ignore [^:]*:? *$' \
  'biome-ignore без объяснения после двоеточия' "$ts"

# allow на весь crate гасит линт во всех файлах разом (тест-крейт — исключение,
# он в server/tests и сюда не попадает)
forbid '^#!\[allow\(' \
  '#![allow(...)] на уровне crate — подавляй точечно в месте проблемы' "$rs_src"
forbid 'allow\(clippy::all' \
  'allow(clippy::all) выключает линтер целиком' "$rs"

# единственная точка чтения окружения — core/config.rs (см. B2 в плане)
env_rs=$(printf '%s\n' $rs_src | grep -v 'server/src/core/config.rs')
forbid 'env::var' \
  'окружение читается только в core/config.rs — добавь поле в Settings' "$env_rs"

exit $fail
