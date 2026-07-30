// Строки разобранной сметы. Показываем и распознанные работы, и то, что
// понять не удалось: нераспознанное — это тоже ответ («спросите бригаду, что
// это»), а не брак разбора, который надо прятать.
import { useQuery } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import type { EstimateDetails, EstimateLine } from '@/api/client'
import { api } from '@/api/client'
import { QueryError } from '@/components/states'

export function EstimateLines({ id }: { id: string }) {
  const { t, i18n } = useTranslation()
  const details = useQuery({
    queryKey: ['estimates', id],
    queryFn: () => api.get<EstimateDetails>(`/api/estimates/${id}`),
  })

  if (details.isPending)
    return <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
  if (details.isError) {
    return <QueryError error={details.error} onRetry={() => void details.refetch()} />
  }

  const lines = details.data.lines
  const recognized = lines.filter((line) => line.title)
  const unknown = lines.filter((line) => !line.title)
  const number = (value: number) => value.toLocaleString(i18n.language)

  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground">
        {t('estimates.recognized', { count: recognized.length })}
      </p>
      <ul className="space-y-2">
        {recognized.map((line) => (
          <li key={line.position} className="border-b pb-2 last:border-0">
            <Work line={line} number={number} />
          </li>
        ))}
      </ul>

      {unknown.length > 0 && (
        <div className="space-y-2">
          <p className="text-sm font-medium">{t('estimates.unknown_title')}</p>
          <p className="text-sm text-muted-foreground">{t('estimates.unknown_hint')}</p>
          <ul className="space-y-1">
            {unknown.map((line) => (
              <li key={line.position} className="text-sm break-words text-muted-foreground">
                {line.raw_text}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  )
}

/** Одна распознанная работа: название, а под ним объём и цены */
function Work({ line, number }: { line: EstimateLine; number: (value: number) => string }) {
  const { t } = useTranslation()
  return (
    <>
      {/* на телефоне сумма уходит под название, но остаётся у правого края */}
      <div className="flex flex-wrap justify-between gap-x-4">
        <span className="break-words">{line.title}</span>
        {line.total != null && <span className="ml-auto tabular-nums">{number(line.total)}</span>}
      </div>
      <p className="flex flex-wrap gap-x-3 text-sm text-muted-foreground">
        {line.quantity != null && (
          <span>
            {t('estimates.quantity', { value: number(line.quantity), unit: line.unit ?? '' })}
          </span>
        )}
        {line.price != null && <span>{t('estimates.price', { value: number(line.price) })}</span>}
      </p>
    </>
  )
}
