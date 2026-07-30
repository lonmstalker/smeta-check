#!/bin/sh
# Stop-хук Claude Code: правило «перед сдачей» из AGENTS.md срабатывает
# механически — в момент, когда агент собрался закончить ход.
# Метку .git/claude-pending-journal ставит hook-check.sh при правке кода
# и снимает при правке docs/journal.md. Блокировка (exit 2) не зацикливается:
# при повторной остановке stop_hook_active=true и хук молчит.
active=$(python3 -c "import json,sys; print(json.load(sys.stdin).get('stop_hook_active', False))" 2>/dev/null)
[ "$active" = "True" ] && exit 0
cd "$(dirname "$0")/.." || exit 0

msg=""
if [ -f .git/claude-pending-journal ]; then
  rm -f .git/claude-pending-journal
  msg="Напоминание перед сдачей: код менялся, а docs/journal.md — нет. Сдаёшь работу — добавь запись в журнал (формат — скилл workflow) и убедись, что make verify зелёный. Работа ещё в процессе — скажи это пользователю и продолжай."
fi

# Тест-контейнеры (testcontainers) должны умирать вместе с тестами;
# dev-база из compose под эту метку не попадает.
orphans=$(docker ps --filter label=org.testcontainers.managed-by --format '{{.ID}} {{.Image}}' 2>/dev/null)
if [ -n "$orphans" ]; then
  msg="$msg
Осиротевшие тест-контейнеры: $orphans — останови их (docker stop <id>)."
fi

if [ -n "$msg" ]; then
  printf '%s\n' "$msg" >&2
  exit 2
fi
exit 0
