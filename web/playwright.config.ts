import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  // Таймауты на всех уровнях: зависший тест или сервер убиваются, ничего не
  // остаётся жить в фоне (globalTimeout — жёсткий потолок всего прогона).
  timeout: 30_000,
  globalTimeout: 300_000,
  expect: { timeout: 5_000 },
  // .only отлаживают локально, но забывают убрать — падаем всегда, не только в CI
  forbidOnly: true,
  // Один повтор в CI даёт две трассы для разбора, но восстановившийся тест —
  // всё равно красный джоб: «иногда падает» лечится, а не терпится.
  retries: process.env.CI ? 1 : 0,
  failOnFlakyTests: !!process.env.CI,
  // В CI HTML-отчёт с трассами уходит артефактом (см. ci.yml) — человек
  // разбирает падение по скриншотам и film strip, не запуская ничего локально.
  reporter: process.env.CI ? [['dot'], ['html', { open: 'never' }]] : [['list']],
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  use: {
    baseURL: 'http://localhost:8081',
    // тесты написаны по русским текстам — язык по умолчанию
    locale: 'ru-RU',
    // приложение уважает «уменьшить движение» (kill-switch в index.css):
    // тесты бегут без анимаций и не кликают по элементу, который ещё «едет»
    reducedMotion: 'reduce',
    trace: 'retain-on-failure',
  },
  webServer: {
    // Playwright сам поднимает полный стек (Postgres + api + собранный фронт)
    // и сам гасит его по окончании тестов.
    command: 'sh ../scripts/e2e-server.sh',
    url: 'http://localhost:8081/api/health',
    reuseExistingServer: !process.env.CI,
    timeout: 240_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
})
