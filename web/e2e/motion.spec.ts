// Проверка kill-switch из index.css: при системной настройке «уменьшить
// движение» (эмулируется конфигом, reducedMotion: 'reduce') анимации
// приложения фактически выключены — а не просто «медиа-запрос объявлен».
import { expect, test } from './fixtures'

test('при «уменьшить движение» анимации выключены', async ({ page }) => {
  await page.goto('/login')

  // kill-switch дожал transition до нуля даже у кнопки с transition-colors
  const duration = await page
    .getByRole('button', { name: 'Войти' })
    .evaluate((el) => getComputedStyle(el).transitionDuration)
  expect(Number.parseFloat(duration)).toBeLessThan(0.01)

  // и на странице нет ни одной идущей анимации
  const running = await page.evaluate(() => document.getAnimations().length)
  expect(running).toBe(0)
})
