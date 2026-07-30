import { screen } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import { guestApi, renderApp } from '../test-utils'
import { ErrorBoundary } from './ErrorBoundary'

afterEach(() => vi.unstubAllGlobals())

function Boom(): never {
  throw new Error('намеренная ошибка рендера')
}

test('ошибка рендера показывает заглушку вместо белого экрана', async () => {
  const { calls } = guestApi({ '/api/logs': () => new Response(null, { status: 202 }) })
  // React пишет пойманную ошибку в console.error — в тесте это ожидаемо
  vi.spyOn(console, 'error').mockImplementation(() => {})
  renderApp(
    <ErrorBoundary>
      <Boom />
    </ErrorBoundary>,
  )
  expect(await screen.findByText('Что-то пошло не так')).toBeInTheDocument()
  // подробности улетели на бэк
  expect(calls).toContain('POST /api/logs')
})
