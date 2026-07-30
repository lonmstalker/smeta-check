import { defineConfig } from '@playwright/test'

// Визуальный слой: собранный фронт + застабленный API (visual.spec.ts) —
// ни Rust, ни БД не нужны, данные детерминированы, маски не требуются.
// Эталоны рендерятся ТОЛЬКО в CI-джобе visual (контейнер Playwright):
// локальный рендер другой ОС/архитектуры с ними не совпадает, поэтому вне
// CI сравнение пропускается (ignoreSnapshots) — тест лишь прогоняет страницы.
export default defineConfig({
  testDir: './e2e',
  testMatch: 'visual.spec.ts',
  timeout: 30_000,
  globalTimeout: 240_000,
  expect: { timeout: 5_000, toHaveScreenshot: { maxDiffPixelRatio: 0.01 } },
  forbidOnly: true,
  // скриншот обязан совпасть с первого раза: «мигающий» дифф чинится,
  // а не пересдаётся ретраем
  retries: 0,
  ignoreSnapshots: !process.env.CI,
  snapshotPathTemplate: '{testDir}/__screenshots__/{projectName}/{arg}{ext}',
  reporter: process.env.CI
    ? [['dot'], ['html', { open: 'never', outputFolder: 'playwright-report-visual' }]]
    : [['list']],
  projects: [
    { name: 'visual-desktop', use: { viewport: { width: 1280, height: 800 } } },
    { name: 'visual-tablet', use: { viewport: { width: 768, height: 1024 } } },
    { name: 'visual-mobile', use: { viewport: { width: 393, height: 852 } } },
  ],
  use: {
    baseURL: 'http://localhost:8082',
    locale: 'ru-RU',
    // фиксируем время: <DateTime> рендерит одинаковые строки в CI и локально
    timezoneId: 'UTC',
    reducedMotion: 'reduce',
  },
  webServer: {
    command: 'pnpm exec vite preview --port 8082 --strictPort',
    url: 'http://localhost:8082',
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
})
