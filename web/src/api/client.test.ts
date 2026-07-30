// Тихое восстановление сессии: 401 из-за протухшего access-токена лечится
// refresh'ем и повтором запроса — в том числе на аккаунтных ручках
// /api/auth/* (сессии, пароль, почта). Ручки самого входа не ретраятся:
// их 401 — «неверные данные». Плюс: один общий refresh на все параллельные
// 401 и честный выход, когда обновиться не вышло.
import { afterEach, expect, test, vi } from 'vitest'
import { mockApi } from '../test-utils'
import { api, setAccessToken, setOnSessionExpired } from './client'

afterEach(() => {
  setAccessToken(null)
  setOnSessionExpired(null)
})

const err401 = () =>
  new Response(JSON.stringify({ error: { code: 'error-unauthorized', message: '' } }), {
    status: 401,
  })

test('протухший токен на списке сессий обновляется, запрос повторяется', async () => {
  let sessionCalls = 0
  mockApi({
    '/api/auth/sessions': () => {
      sessionCalls += 1
      return sessionCalls === 1 ? err401() : [{ id: 'a', current: true }]
    },
    '/api/auth/refresh': () => ({ access_token: 'fresh', user: { id: 'u' } }),
  })
  const sessions = await api.get<unknown[]>('/api/auth/sessions')
  expect(sessions).toHaveLength(1)
  expect(sessionCalls).toBe(2)
})

test('401 при входе не пытается обновить сессию', async () => {
  const { calls } = mockApi({ '/api/auth/login': err401 })
  await expect(api.post('/api/auth/login', { email: 'a@b', password: 'x' })).rejects.toThrow()
  expect(calls).not.toContain('POST /api/auth/refresh')
})

test('параллельные 401 обновляют токен один раз', async () => {
  let refreshed = false
  const { calls } = mockApi({
    // до обновления обе ручки отвечают 401, после — отдают данные
    '/api/items': () => (refreshed ? { items: [] } : err401()),
    '/api/users/me': () => (refreshed ? { id: 'u1' } : err401()),
    '/api/auth/refresh': () => {
      refreshed = true
      return { access_token: 'fresh', user: { id: 'u1' } }
    },
  })

  await Promise.all([api.get('/api/items'), api.get('/api/users/me')])

  expect(calls.filter((c) => c === 'POST /api/auth/refresh')).toHaveLength(1)
})

test('провал обновления сообщает приложению, что сессия кончилась', async () => {
  const expired = vi.fn()
  setOnSessionExpired(expired)
  setAccessToken('stale')
  mockApi({ '/api/items': err401, '/api/auth/refresh': err401 })

  await expect(api.get('/api/items')).rejects.toThrow()
  expect(expired).toHaveBeenCalledTimes(1)
})
