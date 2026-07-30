// Главная страница. Async-first: данные через TanStack Query — кэш,
// фоновое обновление и оптимистичный UI без ручных «крутилок» на каждый чих.
// Записи личные: список приходит только вошедшему и только его собственный.
import { useInfiniteQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import type { Item, ItemsPage as ItemsPageData } from '@/api/client'
import { api } from '@/api/client'
import { useAuth } from '@/auth/AuthContext'
import { Page, PageHeader } from '@/components/AppShell'
import { FieldError, FormError, invalid } from '@/components/form'
import { EmptyState, PendingButton, QueryError } from '@/components/states'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

export default function ItemsPage() {
  const { t } = useTranslation()
  const { user, ready } = useAuth()
  const queryClient = useQueryClient()
  const [title, setTitle] = useState('')

  // постраничная загрузка: сервер отдаёт next_cursor, пока есть продолжение
  const items = useInfiniteQuery({
    queryKey: ['items'],
    enabled: !!user,
    queryFn: ({ pageParam }) =>
      api.get<ItemsPageData>(pageParam ? `/api/items?cursor=${pageParam}` : '/api/items'),
    initialPageParam: null as number | null,
    getNextPageParam: (last) => last.next_cursor ?? null,
  })
  const loaded = items.data?.pages.flatMap((page) => page.items) ?? []

  const create = useMutation({
    mutationFn: (title: string) => api.post<Item>('/api/items', { title }),
    onSuccess: () => {
      setTitle('')
      void queryClient.invalidateQueries({ queryKey: ['items'] })
    },
  })

  const count = loaded.length

  if (!user) {
    return (
      <Page>
        <PageHeader title={t('items.title')} />
        {/* пока сессия восстанавливается, вошедшему не мигает «войдите» */}
        {ready ? (
          <EmptyState text={t('items.login_hint')}>
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

  return (
    <Page>
      <PageHeader title={t('items.title')} />

      <form
        onSubmit={(e) => {
          e.preventDefault()
          create.mutate(title)
        }}
        className="space-y-1"
      >
        <div className="flex gap-2">
          <Input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t('items.placeholder')}
            {...invalid(create.error, 'title')}
          />
          <PendingButton type="submit" pending={create.isPending}>
            {t('items.create')}
          </PendingButton>
        </div>
        <FieldError error={create.error} field="title" />
      </form>
      <FormError error={create.error} />

      <Card>
        <CardHeader>
          {/* count прогоняется через склонения: 1 запись / 2 записи / 5 записей */}
          <CardTitle>{t('items.count', { count })}</CardTitle>
        </CardHeader>
        <CardContent>
          {items.isPending ? (
            <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
          ) : items.isError ? (
            <QueryError error={items.error} onRetry={() => void items.refetch()} />
          ) : count === 0 ? (
            <EmptyState text={t('items.empty')} />
          ) : (
            <div className="space-y-2">
              <ul className="space-y-2">
                {loaded.map((item: Item) => (
                  <li key={item.id} className="rounded-md border p-2">
                    {item.title}
                  </li>
                ))}
              </ul>
              {items.hasNextPage && (
                <PendingButton
                  variant="outline"
                  className="w-full"
                  pending={items.isFetchingNextPage}
                  onClick={() => items.fetchNextPage()}
                >
                  {t('items.more')}
                </PendingButton>
              )}
            </div>
          )}
        </CardContent>
      </Card>
    </Page>
  )
}
