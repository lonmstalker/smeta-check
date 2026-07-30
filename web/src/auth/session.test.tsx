// Сессия глазами интерфейса: смена пользователя, второй фактор и потеря
// сессии. Проверяем через настоящий App — со своим QueryClient (кэш живёт
// 30 секунд), иначе тесты не увидят чужие данные в кэше.
import { fireEvent, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import App from '../App'
import { setAccessToken } from '../api/client'
import { mockApi, renderApp } from '../test-utils'

afterEach(() => {
  vi.unstubAllGlobals()
  setAccessToken(null)
})

function unauthorized() {
  return new Response(JSON.stringify({ error: { code: 'error-unauthorized', message: '' } }), {
    status: 401,
  })
}

const first = {
  id: 'u1',
  email: 'first@test.local',
  role: 'user',
  totp_enabled: false,
  email_verified: true,
}
const second = { ...first, id: 'u2', email: 'second@test.local' }

test('после смены пользователя список берётся заново, а не из чужого кэша', async () => {
  // записи личные: у каждого пользователя свой список за одним и тем же ключом
  let current = first
  mockApi({
    '/api/auth/refresh': () => ({ access_token: 'token', user: current }),
    '/api/items': () => ({ items: [{ id: 1, title: `Запись ${current.id}` }] }),
    '/api/auth/logout': () => new Response(null, { status: 204 }),
    '/api/auth/login': () => {
      current = second
      return { access_token: 'token', user: second, requires_2fa: false }
    },
  })
  renderApp(<App />)
  expect(await screen.findByText('Запись u1')).toBeInTheDocument()

  fireEvent.click(screen.getByRole('button', { name: 'Выйти' }))
  // после выхода попадаем на главную для гостя — оттуда на страницу входа
  fireEvent.click((await screen.findAllByRole('link', { name: 'Войти' }))[0])
  fireEvent.change(await screen.findByPlaceholderText('Почта'), {
    target: { value: second.email },
  })
  fireEvent.change(screen.getByPlaceholderText('Пароль'), { target: { value: 'correct-horse-9' } })
  fireEvent.click(screen.getByRole('button', { name: 'Войти' }))

  expect(await screen.findByText('Запись u2')).toBeInTheDocument()
  expect(screen.queryByText('Запись u1')).not.toBeInTheDocument()
})

test('пока сессия восстанавливается, вошедшему не мигает приглашение войти', () => {
  mockApi({
    // ответ не приходит: интерфейс должен молчать, а не звать войти
    '/api/auth/refresh': () => new Promise(() => {}) as unknown as Response,
    '/api/items': () => ({ items: [] }),
  })
  renderApp(<App />)

  expect(screen.queryByText('Чтобы добавлять записи, войдите в систему')).not.toBeInTheDocument()
})

test('вход с двухфакторной защитой спрашивает код', async () => {
  mockApi({
    '/api/auth/refresh': () => unauthorized(),
    '/api/items': () => ({ items: [] }),
    '/api/auth/login': () => ({ requires_2fa: true, pending_token: 'pending-1' }),
    '/api/auth/2fa/verify': () => ({ access_token: 'token', user: second }),
  })
  renderApp(<App />, { route: '/login' })

  fireEvent.change(await screen.findByPlaceholderText('Почта'), {
    target: { value: second.email },
  })
  fireEvent.change(screen.getByPlaceholderText('Пароль'), { target: { value: 'correct-horse-9' } })
  fireEvent.click(screen.getByRole('button', { name: 'Войти' }))

  fireEvent.change(await screen.findByPlaceholderText('Код'), { target: { value: '123456' } })
  fireEvent.click(screen.getByRole('button', { name: 'Подтвердить' }))

  // вошли: в шапке появилась кнопка выхода
  expect(await screen.findByRole('button', { name: 'Выйти' })).toBeInTheDocument()
})

test('после входа через провайдера с 2FA страница входа сразу просит код', async () => {
  mockApi({
    '/api/auth/refresh': () => unauthorized(),
    '/api/items': () => ({ items: [] }),
  })
  // бэк вернул нас на /login с pending-токеном во фрагменте адреса
  renderApp(<App />, { route: '/login#pending=from-oauth' })

  expect(await screen.findByPlaceholderText('Код')).toBeInTheDocument()
  expect(screen.queryByPlaceholderText('Пароль')).not.toBeInTheDocument()
})

test('отозванная сессия разлогинивает интерфейс', async () => {
  let sessionAlive = true
  mockApi({
    '/api/auth/refresh': () =>
      sessionAlive ? { access_token: 'token', user: second } : unauthorized(),
    '/api/items': (_url, init) =>
      init?.method === 'POST' && !sessionAlive ? unauthorized() : { items: [] },
  })
  renderApp(<App />)
  expect(await screen.findByRole('button', { name: 'Выйти' })).toBeInTheDocument()

  // сессию отозвали на сервере: и запрос, и обновление токена дают 401
  sessionAlive = false
  fireEvent.change(screen.getByPlaceholderText('Название'), { target: { value: 'Заметка' } })
  fireEvent.click(screen.getByRole('button', { name: 'Создать' }))

  // шапка перестала показывать вошедшего — интерфейс сам вышел
  await waitFor(() =>
    expect(screen.queryByRole('button', { name: 'Выйти' })).not.toBeInTheDocument(),
  )
})
