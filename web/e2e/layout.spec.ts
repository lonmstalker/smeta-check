// Схемы ключевых страниц. Ожидаемая структура описана руками как
// ARIA-дерево (роли и подписи в порядке DOM) и сверяется тестом:
// пропажа поля или перенос блока в разметке — красный тест с YAML-диффом.
// Semantics subset: лишний элемент тест не ломает; геометрию и «где это
// на экране» проверяют скриншоты (visual-джоб), не эта схема.
import { expect, test } from './fixtures'

test('страница входа устроена как задумано', async ({ page }) => {
  await page.goto('/login')
  await expect(page.locator('body')).toMatchAriaSnapshot(`
    - banner:
      - navigation:
        - link "Business Project"
        - button "Сменить тему"
        - button "EN"
        - link "Войти"
    - main:
      - text: Вход
      - textbox "Почта"
      - textbox "Пароль"
      - button "Войти"
      - link "Войти через VK"
      - link "Войти через Яндекс"
      - paragraph:
        - link "Зарегистрироваться"
        - link "Забыли пароль?"
  `)
})

test('главная вошедшего: шапка с настройками, форма загрузки сметы', async ({
  page,
  user: _user,
}) => {
  await page.goto('/')
  await expect(page.locator('body')).toMatchAriaSnapshot(`
    - banner:
      - navigation:
        - link "Business Project"
        - button "Сменить тему"
        - button "EN"
        - link "Настройки"
        - button "Выйти"
    - main:
      - heading "Мои сметы" [level=1]
      - button "Загрузить"
  `)
})

test('настройки: все разделы аккаунта на месте и по порядку', async ({ page, user: _user }) => {
  await page.goto('/settings')
  // схема снимается с main: шапку проверяет тест главной, здесь важны разделы
  await expect(page.locator('main')).toMatchAriaSnapshot(`
    - main:
      - heading "Настройки" [level=1]
      - text: Профиль
      - textbox "Как к вам обращаться"
      - button "Сохранить"
      - text: Пароль
      - textbox "Текущий пароль"
      - textbox "Новый пароль"
      - button "Сменить пароль"
      - text: Адрес почты
      - textbox "Новый адрес"
      - textbox "Пароль для подтверждения"
      - button "Отправить подтверждение"
      - text: "Двухфакторная защита: Выключена"
      - text: Устройства и сессии
  `)
})
