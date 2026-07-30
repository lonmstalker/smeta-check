// Тёмная тема — класс .dark на <html> (токены уже в index.css).
// Выбор хранится в localStorage; без выбора следуем системной настройке.
const KEY = 'theme'

export function applyStoredTheme() {
  const stored = localStorage.getItem(KEY)
  const dark = stored
    ? stored === 'dark'
    : window.matchMedia('(prefers-color-scheme: dark)').matches
  document.documentElement.classList.toggle('dark', dark)
}

export function toggleTheme() {
  const dark = !document.documentElement.classList.contains('dark')
  localStorage.setItem(KEY, dark ? 'dark' : 'light')
  document.documentElement.classList.toggle('dark', dark)
}
