// Хелперы юнит-тестов: рендер приложения на нужном адресе и мок API.
import { render } from '@testing-library/react'
import { MemoryRouter } from 'react-router'
import { vi } from 'vitest'
import './lib/i18n'
import i18n from './lib/i18n'

/**
 * Рендер в окружении приложения. Провайдеры (query, auth) свои заводит сам
 * App — второй комплект поверх него ломал бы тесты незаметно: интерфейс
 * читал бы одно состояние входа, а клиент API правил другое.
 */
export function renderApp(ui: React.ReactNode, { route = '/' } = {}) {
  void i18n.changeLanguage('ru')
  return render(<MemoryRouter initialEntries={[route]}>{ui}</MemoryRouter>)
}

type Responder = (url: string, init?: RequestInit) => unknown

/**
 * Мок сети: по одному ответчику на путь. Незамоканный путь = ошибка 500 —
 * тест сразу видит неожиданный запрос вместо тихого зависания.
 */
export function mockApi(routes: Record<string, Responder>) {
  const calls: string[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = new URL(String(input), 'http://test.local').pathname
      calls.push(`${init?.method ?? 'GET'} ${url}`)
      const responder = routes[url]
      if (!responder) {
        return new Response(JSON.stringify({ error: { code: 'unmocked', message: url } }), {
          status: 500,
        })
      }
      const result = responder(url, init)
      if (result instanceof Response) return result
      return new Response(JSON.stringify(result), {
        headers: { 'content-type': 'application/json' },
      })
    }),
  )
  return { calls }
}

/** Стандартные ответы: гость (нет сессии) */
export function guestApi(extra: Record<string, Responder> = {}) {
  return mockApi({
    '/api/auth/refresh': () =>
      new Response(JSON.stringify({ error: { code: 'error-unauthorized', message: '' } }), {
        status: 401,
      }),
    ...extra,
  })
}

/** Стандартные ответы: обычный вошедший пользователь */
export function authedApi(extra: Record<string, Responder> = {}) {
  const user = {
    id: '00000000-0000-0000-0000-000000000001',
    email: 'user@test.local',
    role: 'user',
    totp_enabled: false,
    email_verified: true,
  }
  return mockApi({
    '/api/auth/refresh': () => ({ access_token: 'test-access-token', user }),
    ...extra,
  })
}
