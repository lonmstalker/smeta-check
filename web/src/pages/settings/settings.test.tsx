import { fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import App from '@/App'
import { setAccessToken } from '@/api/client'
import { authedApi, renderApp } from '@/test-utils'

afterEach(() => {
  vi.unstubAllGlobals()
  setAccessToken(null)
})

const SESSIONS = [
  {
    id: '11111111-1111-1111-1111-111111111111',
    created_at: '2026-07-20T09:00:00Z',
    last_seen_at: '2026-07-24T18:30:00Z',
    client: 'Chrome, macOS',
    current: true,
  },
  {
    id: '22222222-2222-2222-2222-222222222222',
    created_at: '2026-07-01T09:00:00Z',
    last_seen_at: '2026-07-02T10:00:00Z',
    client: 'Firefox, Linux',
    current: false,
  },
]

test('в настройках видны устройства, чужое можно закрыть', async () => {
  let sessions = SESSIONS
  const { calls } = authedApi({
    '/api/auth/sessions': (_url, init) => {
      if (init?.method === 'DELETE') {
        sessions = sessions.filter((s) => s.current)
        return new Response(null, { status: 204 })
      }
      return sessions
    },
  })
  renderApp(<App />, { route: '/settings' })

  expect(await screen.findByText(/Chrome, macOS/)).toBeInTheDocument()
  expect(screen.getByText(/Firefox, Linux/)).toBeInTheDocument()
  // время с бэка (UTC) показано в местном формате, а не сырой строкой
  expect(screen.queryByText('2026-07-24T18:30:00Z')).not.toBeInTheDocument()

  fireEvent.click(screen.getByRole('button', { name: 'Выйти на других устройствах' }))
  await waitFor(() => expect(screen.queryByText(/Firefox, Linux/)).not.toBeInTheDocument())
  expect(calls).toContain('DELETE /api/auth/sessions')
})

test('смена пароля просит текущий и показывает ошибку у своего поля', async () => {
  authedApi({
    '/api/auth/sessions': () => [],
    '/api/auth/password': () =>
      new Response(
        JSON.stringify({
          error: {
            code: 'error-wrong-password',
            message: 'Текущий пароль указан неверно',
            fields: [
              {
                field: 'current_password',
                code: 'error-wrong-password',
                message: 'Текущий пароль указан неверно',
              },
            ],
          },
        }),
        { status: 422, headers: { 'content-type': 'application/json' } },
      ),
  })
  renderApp(<App />, { route: '/settings' })

  fireEvent.change(await screen.findByPlaceholderText('Текущий пароль'), {
    target: { value: 'не тот' },
  })
  fireEvent.change(screen.getByPlaceholderText('Новый пароль'), {
    target: { value: 'new-horse-9' },
  })
  fireEvent.click(screen.getByRole('button', { name: 'Сменить пароль' }))

  expect(await screen.findByRole('alert')).toHaveTextContent('Текущий пароль указан неверно')
})
