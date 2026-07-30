import { expect, test } from './fixtures'

// Полный путь пользователя через настоящий стек: браузер -> фронт -> axum -> Postgres
test('регистрация, создание записи, выход и повторный вход', async ({ page, user }) => {
  const title = `Запись ${Date.now()}`
  await page.getByPlaceholder('Название').fill(title)
  await page.getByRole('button', { name: 'Создать', exact: true }).click()
  await expect(page.getByText(title)).toBeVisible()

  await page.getByRole('button', { name: 'Выйти' }).click()
  // «Войти» теперь и в шапке, и в пустом состоянии главной — берём шапку
  const loginLink = page.getByRole('navigation').getByRole('link', { name: 'Войти' })
  await expect(loginLink).toBeVisible()

  await loginLink.click()
  await page.getByPlaceholder('Почта').fill(user.email)
  await page.getByPlaceholder('Пароль').fill(user.password)
  await page.getByRole('button', { name: 'Войти', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Выйти' })).toBeVisible()
  await expect(page.getByText(title)).toBeVisible()
})

test('интерфейс переключается на английский', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Записи' })).toBeVisible()
  await page.getByRole('button', { name: 'EN' }).click()
  await expect(page.getByRole('heading', { name: 'Items' })).toBeVisible()
})
