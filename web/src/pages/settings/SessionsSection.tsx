import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import type { SessionInfo } from '@/api/client'
import { api } from '@/api/client'
import { DateTime } from '@/components/DateTime'
import { EmptyState, PendingButton, QueryError } from '@/components/states'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

/** Где ещё открыт аккаунт — и кнопка закрыть лишнее */
export function SessionsSection() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const sessions = useQuery({
    queryKey: ['sessions'],
    queryFn: () => api.get<SessionInfo[]>('/api/auth/sessions'),
  })
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: ['sessions'] })
  const revoke = useMutation({
    mutationFn: (id: string) => api.delete(`/api/auth/sessions/${id}`),
    onSuccess: invalidate,
  })
  const revokeOthers = useMutation({
    mutationFn: () => api.delete('/api/auth/sessions'),
    onSuccess: invalidate,
  })

  const list = sessions.data ?? []
  const others = list.filter((s) => !s.current).length

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('settings.sessions.title')}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {sessions.isPending ? (
          <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
        ) : sessions.isError ? (
          <QueryError error={sessions.error} onRetry={() => void sessions.refetch()} />
        ) : list.length === 0 ? (
          <EmptyState text={t('settings.sessions.empty')} />
        ) : (
          <ul className="space-y-2">
            {list.map((session) => (
              <li
                key={session.id}
                className="flex items-center justify-between gap-4 rounded-md border p-2"
              >
                <div className="space-y-0.5 text-sm">
                  <p>
                    {session.client ?? t('settings.sessions.unknown_client')}
                    {session.current ? ` — ${t('settings.sessions.current')}` : ''}
                  </p>
                  <p className="text-xs text-muted-foreground">
                    {t('settings.sessions.last_seen')} <DateTime value={session.last_seen_at} />
                  </p>
                </div>
                {!session.current && (
                  <PendingButton
                    variant="outline"
                    size="sm"
                    pending={revoke.isPending}
                    onClick={() => revoke.mutate(session.id)}
                  >
                    {t('settings.sessions.revoke')}
                  </PendingButton>
                )}
              </li>
            ))}
          </ul>
        )}
        {others > 0 && (
          <PendingButton
            variant="destructive"
            pending={revokeOthers.isPending}
            onClick={() => revokeOthers.mutate()}
          >
            {t('settings.sessions.revoke_others')}
          </PendingButton>
        )}
      </CardContent>
    </Card>
  )
}
