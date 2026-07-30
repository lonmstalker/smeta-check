import { expect, test } from './fixtures'

// Настоящая смета из фикстур бэкенда: тот же файл разбирают и тесты сервера
const ESTIMATE_FILE = '../server/tests/fixtures/estimates/elektromontazh-nekrasov.xlsx'

// Полный путь пользователя через настоящий стек: браузер -> фронт -> axum ->
// Postgres -> фоновый разбор файла
test('регистрация, загрузка сметы и разбор на строки', async ({ page, user: _user }) => {
  await page.getByLabel('Файл сметы').setInputFiles(ESTIMATE_FILE)
  await page.getByRole('button', { name: 'Загрузить' }).click()
  await expect(page.getByText('elektromontazh-nekrasov.xlsx')).toBeVisible()

  // разбор идёт в фоне; страница сама обновляется, пока смета не готова
  const showLines = page.getByRole('button', { name: 'Показать строки' })
  await expect(showLines).toBeVisible({ timeout: 15_000 })
  await showLines.click()
  await expect(page.getByText('Демонтаж светильников')).toBeVisible()
})

test('на телефоне страница не уезжает вбок', async ({ page, user: _user }) => {
  await page.setViewportSize({ width: 375, height: 812 })
  await page.getByLabel('Файл сметы').setInputFiles(ESTIMATE_FILE)
  await page.getByRole('button', { name: 'Загрузить' }).click()
  const showLines = page.getByRole('button', { name: 'Показать строки' })
  await expect(showLines).toBeVisible({ timeout: 15_000 })
  await showLines.click()
  await expect(page.getByText('Демонтаж светильников')).toBeVisible()

  // горизонтальная прокрутка на телефоне — это сломанная вёрстка
  const overflow = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  )
  expect(overflow).toBeLessThanOrEqual(0)
})

test('интерфейс переключается на английский', async ({ page }) => {
  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Мои сметы' })).toBeVisible()
  await page.getByRole('button', { name: 'EN' }).click()
  await expect(page.getByRole('heading', { name: 'My estimates' })).toBeVisible()
})
