import { fireEvent, screen } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import App from './App'
import { setAccessToken } from './api/client'
import { authedApi, guestApi, renderApp } from './test-utils'

afterEach(() => {
  vi.unstubAllGlobals()
  setAccessToken(null)
})

test('гость не видит чужих записей, а видит приглашение войти', async () => {
  guestApi({
    '/api/items': () => ({ items: [{ id: 1, title: 'Первая запись' }] }),
  })
  renderApp(<App />)
  expect(await screen.findByText('Чтобы добавлять записи, войдите в систему')).toBeInTheDocument()
  expect(screen.queryByText('Первая запись')).not.toBeInTheDocument()
})

test('вошедший видит свои записи', async () => {
  authedApi({
    '/api/items': () => ({ items: [{ id: 1, title: 'Первая запись' }] }),
  })
  renderApp(<App />)
  expect(await screen.findByText('Первая запись')).toBeInTheDocument()
  // 1 — единственное число: «запись», не «записей»
  expect(await screen.findByText('1 запись')).toBeInTheDocument()
})

test('склонения работают: 5 записей', async () => {
  authedApi({
    '/api/items': () => ({
      items: [1, 2, 3, 4, 5].map((id) => ({ id, title: `Запись ${id}` })),
    }),
  })
  renderApp(<App />)
  expect(await screen.findByText('5 записей')).toBeInTheDocument()
})

test('кнопка «Показать ещё» подгружает следующую страницу', async () => {
  // одна ручка, разные страницы: первая — с курсором, вторая — последняя
  let call = 0
  authedApi({
    '/api/items': () =>
      call++ === 0
        ? { items: [{ id: 1, title: 'Старая запись' }], next_cursor: 1 }
        : { items: [{ id: 2, title: 'Новая запись' }] },
  })
  renderApp(<App />)
  expect(await screen.findByText('Старая запись')).toBeInTheDocument()

  fireEvent.click(screen.getByRole('button', { name: 'Показать ещё' }))
  expect(await screen.findByText('Новая запись')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Показать ещё' })).not.toBeInTheDocument()
})

test('неизвестный адрес показывает страницу 404', async () => {
  guestApi()
  renderApp(<App />, { route: '/no-such-page' })
  expect(await screen.findByText('Такой страницы нет')).toBeInTheDocument()
})

test('страница входа открывается по /login', async () => {
  guestApi()
  renderApp(<App />, { route: '/login' })
  expect(await screen.findByRole('button', { name: 'Войти' })).toBeInTheDocument()
  expect(screen.getByPlaceholderText('Почта')).toBeInTheDocument()
  expect(screen.getByPlaceholderText('Пароль')).toBeInTheDocument()
})
