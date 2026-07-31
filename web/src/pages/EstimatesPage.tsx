// Главная страница: свои сметы и загрузка новой. Данные — через TanStack
// Query; пока хотя бы одна смета разбирается, список сам обновляется, поэтому
// человек видит результат, ничего не нажимая.
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import type { Estimate } from '@/api/client'
import { api } from '@/api/client'
import { useAuth } from '@/auth/AuthContext'
import { Page, PageHeader } from '@/components/AppShell'
import { DateTime } from '@/components/DateTime'
import { FormError } from '@/components/form'
import { EmptyState, PendingButton, QueryError } from '@/components/states'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { EstimateLines } from '@/pages/estimates/EstimateLines'

/// пока смета в очереди или разбирается — спрашиваем сервер раз в две секунды
const POLL_MS = 2_000
const IN_PROGRESS = ['uploaded', 'parsing']

export default function EstimatesPage() {
  const { t } = useTranslation()
  const { user, ready } = useAuth()
  const queryClient = useQueryClient()
  const fileInput = useRef<HTMLInputElement>(null)
  const [openId, setOpenId] = useState<string | null>(null)

  const estimates = useQuery({
    queryKey: ['estimates'],
    enabled: !!user,
    queryFn: () => api.get<Estimate[]>('/api/estimates'),
    refetchInterval: (query) =>
      query.state.data?.some((e) => IN_PROGRESS.includes(e.status)) ? POLL_MS : false,
  })

  const upload = useMutation({
    mutationFn: (file: File) => {
      const form = new FormData()
      form.append('file', file)
      return api.postForm<Estimate>('/api/estimates', form)
    },
    onSuccess: () => {
      if (fileInput.current) fileInput.current.value = ''
      void queryClient.invalidateQueries({ queryKey: ['estimates'] })
    },
  })

  if (!user) {
    return (
      <Page>
        <PageHeader title={t('estimates.title')} />
        {/* пока сессия восстанавливается, вошедшему не мигает «войдите» */}
        {ready ? (
          <EmptyState text={t('estimates.login_hint')}>
            <Button size="sm" render={<Link to="/login" />}>
              {t('nav.login')}
            </Button>
          </EmptyState>
        ) : (
          <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
        )}
      </Page>
    )
  }

  const list = estimates.data ?? []

  return (
    <Page>
      <PageHeader title={t('estimates.title')} />

      <form
        onSubmit={(e) => {
          e.preventDefault()
          const file = fileInput.current?.files?.[0]
          if (file) upload.mutate(file)
        }}
        className="space-y-2"
      >
        <div className="flex flex-col gap-2 sm:flex-row">
          <Input
            ref={fileInput}
            type="file"
            /* фото — по MIME-типам: так iOS сам переводит HEIC из галереи в JPEG */
            accept=".xlsx,.xls,image/jpeg,image/png,image/webp"
            aria-label={t('estimates.file')}
            className="h-auto py-1.5"
          />
          <PendingButton type="submit" pending={upload.isPending} className="sm:w-auto">
            {t('estimates.upload')}
          </PendingButton>
        </div>
        <p className="text-sm text-muted-foreground">{t('estimates.hint')}</p>
        <FormError error={upload.error} />
      </form>

      {estimates.isPending ? (
        <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
      ) : estimates.isError ? (
        <QueryError error={estimates.error} onRetry={() => void estimates.refetch()} />
      ) : list.length === 0 ? (
        <EmptyState text={t('estimates.empty')} />
      ) : (
        <ul className="space-y-3">
          {list.map((estimate) => (
            <li key={estimate.id}>
              <EstimateCard
                estimate={estimate}
                open={openId === estimate.id}
                onToggle={() => setOpenId(openId === estimate.id ? null : estimate.id)}
              />
            </li>
          ))}
        </ul>
      )}
    </Page>
  )
}

function EstimateCard({
  estimate,
  open,
  onToggle,
}: {
  estimate: Estimate
  open: boolean
  onToggle: () => void
}) {
  const { t } = useTranslation()
  const ready = estimate.status === 'parsed'
  return (
    <Card>
      <CardHeader>
        {/* длинное имя файла не должно распирать карточку на телефоне */}
        <CardTitle className="break-words">{estimate.file_name}</CardTitle>
        <p className="flex flex-wrap gap-x-3 text-sm text-muted-foreground">
          <span>{t(`estimates.status.${estimate.status}`)}</span>
          <DateTime value={estimate.created_at} />
        </p>
        {estimate.error && <p className="text-sm text-destructive">{estimate.error}</p>}
      </CardHeader>
      {ready && (
        <CardContent className="space-y-3">
          <Button variant="outline" size="sm" onClick={onToggle}>
            {open ? t('estimates.hide_lines') : t('estimates.show_lines')}
          </Button>
          {open && <EstimateLines id={estimate.id} />}
        </CardContent>
      )}
    </Card>
  )
}
