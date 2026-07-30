import { screen } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import App from '@/App'
import { setAccessToken } from '@/api/client'
import { authedApi, guestApi, renderApp } from '@/test-utils'

afterEach(() => {
  vi.unstubAllGlobals()
  setAccessToken(null)
})

test('гостя со страницы настроек уводит на вход', async () => {
  guestApi()
  renderApp(<App />, { route: '/settings' })
  expect(await screen.findByText('Вход')).toBeInTheDocument()
  expect(screen.queryByText('Настройки')).not.toBeInTheDocument()
})

test('вошедший попадает на страницу настроек', async () => {
  authedApi()
  renderApp(<App />, { route: '/settings' })
  expect(await screen.findByRole('heading', { name: 'Настройки' })).toBeInTheDocument()
})
