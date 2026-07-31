// Скриншот-эталоны трёх ширин (проекты visual-* из playwright.visual.config).
// API застаблен прямо в браузере: бэкенд не нужен, данные всегда одни и те
// же — эталоны стабильны без масок. Эталоны лежат в e2e/__screenshots__ и
// рендерятся только CI-джобом visual; наезд или переполнение на любой
// ширине — красный джоб с картинкой-диффом в артефакте.
import type { Page } from '@playwright/test'
import { expect, test } from '@playwright/test'

const USER = {
  id: '00000000-0000-0000-0000-000000000001',
  email: 'user@example.com',
  display_name: 'Аня',
  locale: 'ru',
  role: 'user',
  email_verified: true,
  totp_enabled: false,
}

const ESTIMATES = [
  {
    id: '00000000-0000-0000-0000-0000000000e1',
    file_name: 'Смета бригады.xlsx',
    size_bytes: 15403,
    status: 'parsed',
    from_photo: false,
    created_at: '2026-07-20T10:00:00Z',
  },
  {
    id: '00000000-0000-0000-0000-0000000000e2',
    file_name: 'Смета от соседей.xls',
    size_bytes: 34816,
    status: 'parsing',
    from_photo: false,
    created_at: '2026-07-19T09:00:00Z',
  },
  {
    id: '00000000-0000-0000-0000-0000000000e3',
    file_name: 'Смета с телефона.jpg',
    size_bytes: 2048,
    status: 'failed',
    from_photo: true,
    error:
      'Не получилось прочитать смету на фотографии — переснимите её при хорошем свете, чтобы в кадр попали все строки',
    created_at: '2026-07-18T08:00:00Z',
  },
]

const SESSIONS = [
  {
    id: '00000000-0000-0000-0000-0000000000a1',
    client: 'Chrome, macOS',
    current: true,
    created_at: '2026-07-20T10:00:00Z',
    last_seen_at: '2026-07-25T09:30:00Z',
  },
  {
    id: '00000000-0000-0000-0000-0000000000a2',
    client: 'Safari, iPhone',
    current: false,
    created_at: '2026-07-18T08:00:00Z',
    last_seen_at: '2026-07-24T21:00:00Z',
  },
]

async function stubApi(page: Page, { authed }: { authed: boolean }) {
  await page.route('**/api/**', (route) => {
    const { pathname } = new URL(route.request().url())
    const json = (body: unknown, status = 200) =>
      route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) })

    if (pathname === '/api/auth/refresh') {
      return authed
        ? json({ access_token: 'visual-token', user: USER })
        : json({ error: { code: 'unauthorized', message: 'нет сессии' } }, 401)
    }
    if (pathname === '/api/estimates') return json(ESTIMATES)
    if (pathname === '/api/auth/sessions') return json(SESSIONS)
    // незастабленный путь показывает ошибку страницы — её видно на скриншоте
    return json({ error: { code: 'visual_stub_missing', message: pathname } }, 500)
  })
}

test('вход: форма и кнопки провайдеров', async ({ page }) => {
  await stubApi(page, { authed: false })
  await page.goto('/login')
  await expect(page.getByRole('link', { name: 'Войти через VK' })).toBeVisible()
  await expect(page).toHaveScreenshot('login.png', { fullPage: true })
})

test('главная: список смет и форма загрузки', async ({ page }) => {
  await stubApi(page, { authed: true })
  await page.goto('/')
  await expect(page.getByText('Смета от соседей.xls')).toBeVisible()
  await expect(page).toHaveScreenshot('estimates.png', { fullPage: true })
})

test('настройки: все разделы и список устройств', async ({ page }) => {
  await stubApi(page, { authed: true })
  await page.goto('/settings')
  await expect(page.getByRole('button', { name: 'Выйти на других устройствах' })).toBeVisible()
  await expect(page).toHaveScreenshot('settings.png', { fullPage: true })
})
