// Фикстуры e2e: тест со свежим зарегистрированным пользователем начинается
// уже входом в систему — сценарии не дублируют шаги регистрации.
import { test as base, expect } from '@playwright/test'

export type TestUser = { email: string; password: string }

export const test = base.extend<{ user: TestUser }>({
  // reducedMotion из конфига до контекста не доезжает (проверено: matchMedia
  // в странице отвечает false), поэтому включаем настройку явно — тесты не
  // кликают по элементу, который ещё «едет»
  page: async ({ page }, use) => {
    await page.emulateMedia({ reducedMotion: 'reduce' })
    await use(page)
  },
  user: async ({ page }, use) => {
    const user: TestUser = {
      email: `e2e-${Date.now()}-${Math.random().toString(36).slice(2, 8)}@test.local`,
      password: 'correct-horse-9',
    }
    await page.goto('/register')
    await page.getByPlaceholder('Почта').fill(user.email)
    await page.getByPlaceholder('Пароль').fill(user.password)
    await page.getByRole('button', { name: 'Создать аккаунт' }).click()
    await expect(page.getByRole('button', { name: 'Выйти' })).toBeVisible()
    await use(user)
  },
})

export { expect }
