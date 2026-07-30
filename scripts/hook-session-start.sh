#!/bin/sh
# SessionStart-хук Claude Code: ориентировка «где мы» в начале сессии,
# чтобы агент не начинал холодным. Stdout попадает в контекст агента.
cd "$(dirname "$0")/.." || exit 0

echo "## Ориентировка по проекту (SessionStart-хук)"
echo
echo "Последние коммиты:"
git log --oneline -3 2>/dev/null
echo
echo "Последняя запись журнала (docs/journal.md):"
awk '/^## /{n++} n==1' docs/journal.md 2>/dev/null | head -25
echo
plans=$(ls docs/plans/*.md 2>/dev/null)
if [ -n "$plans" ]; then
  echo "Живые планы (docs/plans/):"
  for p in $plans; do
    status=$(grep -m1 -iE 'статус|ЧЕРНОВИК|В РАБОТЕ' "$p" 2>/dev/null)
    echo "- $p — ${status:-статус не указан}"
  done
else
  echo "Живых планов нет (docs/plans/ пуст)."
fi
exit 0
