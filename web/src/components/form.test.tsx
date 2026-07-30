import { fireEvent, screen } from '@testing-library/react'
import { afterEach, expect, test, vi } from 'vitest'
import App from '@/App'
import { setAccessToken } from '@/api/client'
import { guestApi, renderApp } from '@/test-utils'

afterEach(() => {
  vi.unstubAllGlobals()
  setAccessToken(null)
})

/** Ответ сервера про конкретное поле формы */
function fieldErrorResponse(field: string, code: string, message: string) {
  return new Response(
    JSON.stringify({ error: { code, message, fields: [{ field, code, message }] } }),
    { status: 422, headers: { 'content-type': 'application/json' } },
  )
}

test('короткий пароль подсвечивает поле пароля, а не всю форму', async () => {
  guestApi({
    '/api/auth/register': () =>
      fieldErrorResponse(
        'password',
        'error-password-short',
        'Пароль должен быть не короче 8 символов',
      ),
  })
  renderApp(<App />, { route: '/register' })

  fireEvent.change(await screen.findByPlaceholderText('Почта'), {
    target: { value: 'user@example.com' },
  })
  const password = screen.getByPlaceholderText('Пароль')
  fireEvent.change(password, { target: { value: 'коротко' } })
  fireEvent.click(screen.getByRole('button', { name: 'Создать аккаунт' }))

  // сообщение видно один раз (под полем), и само поле помечено неверным
  const alerts = await screen.findAllByRole('alert')
  expect(alerts).toHaveLength(1)
  expect(alerts[0]).toHaveTextContent('Пароль должен быть не короче 8 символов')
  expect(password).toHaveAttribute('aria-invalid', 'true')
  expect(screen.getByPlaceholderText('Почта')).toHaveAttribute('aria-invalid', 'false')
})

test('ошибка без привязки к полю показывается под формой целиком', async () => {
  guestApi({
    '/api/auth/login': () =>
      new Response(
        JSON.stringify({
          error: { code: 'error-invalid-credentials', message: 'Неверная почта или пароль' },
        }),
        { status: 401, headers: { 'content-type': 'application/json' } },
      ),
  })
  renderApp(<App />, { route: '/login' })

  fireEvent.change(await screen.findByPlaceholderText('Почта'), {
    target: { value: 'user@example.com' },
  })
  fireEvent.change(screen.getByPlaceholderText('Пароль'), { target: { value: 'whatever-9' } })
  fireEvent.click(screen.getByRole('button', { name: /^Войти$/ }))

  expect(await screen.findByRole('alert')).toHaveTextContent('Неверная почта или пароль')
})
