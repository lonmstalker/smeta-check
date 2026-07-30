// Три состояния, которые есть у любой страницы с данными: пусто, ошибка
// загрузки, идёт запрос. Раньше каждая страница описывала их сама — теперь
// они выглядят одинаково везде.
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'

/** Данных нет — и это нормально: объясняем, что делать дальше */
export function EmptyState({ text, children }: { text: string; children?: React.ReactNode }) {
  return (
    <div className="space-y-3 py-6 text-center">
      <p className="text-sm text-muted-foreground">{text}</p>
      {children}
    </div>
  )
}

/** Данные не загрузились: показываем причину и даём повторить, а не пустоту */
export function QueryError({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
  const { t } = useTranslation()
  if (!error) return null
  const message = error instanceof Error ? error.message : t('error.title')
  return (
    <div role="alert" className="space-y-3 py-4 text-center">
      <p className="text-sm text-destructive">{message}</p>
      {onRetry && (
        <Button variant="outline" size="sm" onClick={onRetry}>
          {t('common.retry')}
        </Button>
      )}
    </div>
  )
}

/**
 * Кнопка, которая сама блокируется на время запроса: двойной клик по «Создать»
 * не должен создавать две записи.
 */
export function PendingButton({
  pending,
  disabled,
  children,
  ...props
}: React.ComponentProps<typeof Button> & { pending?: boolean }) {
  return (
    <Button disabled={pending || disabled} {...props}>
      {children}
    </Button>
  )
}
