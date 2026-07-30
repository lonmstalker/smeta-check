import { expect, test } from './fixtures'

test('пользователь меняет имя и видит своё устройство в списке сессий', async ({
  page,
  user: _user,
}) => {
  await page.getByRole('link', { name: 'Настройки' }).click()

  await page.getByPlaceholder('Как к вам обращаться').fill('Аня')
  await page.getByRole('button', { name: 'Сохранить' }).click()
  await expect(page.getByText('Сохранено')).toBeVisible()

  // список сессий показывает текущее устройство и время последнего входа
  await expect(page.getByText('это устройство')).toBeVisible()
})

test('смена пароля закрывает сессию и требует войти заново', async ({ page, user }) => {
  await page.getByRole('link', { name: 'Настройки' }).click()

  await page.getByPlaceholder('Текущий пароль').fill(user.password)
  await page.getByPlaceholder('Новый пароль').fill('brand-new-horse-9')
  await page.getByRole('button', { name: 'Сменить пароль' }).click()

  // сессии закрыты — приложение само привело на страницу входа
  await expect(page.getByText('Вход', { exact: true })).toBeVisible()

  await page.getByPlaceholder('Почта').fill(user.email)
  await page.getByPlaceholder('Пароль').fill('brand-new-horse-9')
  await page.getByRole('button', { name: 'Войти', exact: true }).click()
  await expect(page.getByRole('button', { name: 'Выйти' })).toBeVisible()
})
