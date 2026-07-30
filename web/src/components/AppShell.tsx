// Каркас страниц: шапка приложения и колонка контента одной ширины.
// Все обычные страницы выглядят одинаково, потому что берут отсюда, а не
// повторяют разметку у себя.
import { Moon } from 'lucide-react'
import { useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useNavigate } from 'react-router'
import { api } from '@/api/client'
import { useAuth } from '@/auth/AuthContext'
import { Button } from '@/components/ui/button'
import { log } from '@/lib/logger'
import { toggleTheme } from '@/lib/theme'

export function AppShell({ children }: { children: React.ReactNode }) {
  return (
    <>
      <Header />
      {children}
    </>
  )
}

/** Колонка контента обычной страницы */
export function Page({ children }: { children: React.ReactNode }) {
  return <main className="mx-auto max-w-xl space-y-6 p-8">{children}</main>
}

/** Заголовок страницы; справа можно поставить кнопку действия */
export function PageHeader({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <h1 className="text-3xl font-bold">{title}</h1>
      {children}
    </div>
  )
}

function Header() {
  const { t, i18n } = useTranslation()
  const { user, logout, refreshUser } = useAuth()
  const navigate = useNavigate()
  const nextLang = i18n.language.startsWith('ru') ? 'en' : 'ru'

  // Язык, выбранный в аккаунте, важнее языка браузера: пользователь уже
  // сказал, на каком языке хочет читать — и письма придут на нём же.
  useEffect(() => {
    if (user?.locale && !i18n.language.startsWith(user.locale)) {
      void i18n.changeLanguage(user.locale)
    }
  }, [user?.locale, i18n])

  const switchLanguage = async () => {
    await i18n.changeLanguage(nextLang)
    if (!user) return
    try {
      await api.patch('/api/users/me', { locale: nextLang })
      await refreshUser()
    } catch (err) {
      // переключение уже произошло; не сохранилось — не повод ломать страницу
      log.warn('не удалось сохранить язык в профиле', { error: String(err) })
    }
  }

  return (
    <header className="border-b">
      <nav className="mx-auto flex max-w-xl items-center justify-between p-4">
        <Link to="/" className="font-semibold">
          {t('app.title')}
        </Link>
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="sm" aria-label={t('nav.theme')} onClick={toggleTheme}>
            <Moon className="size-4" />
          </Button>
          <Button variant="ghost" size="sm" onClick={() => void switchLanguage()}>
            {nextLang.toUpperCase()}
          </Button>
          {user ? (
            <>
              <Button variant="ghost" size="sm" render={<Link to="/settings" />}>
                {t('nav.settings')}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => logout().then(() => navigate('/'))}
              >
                {t('nav.logout')}
              </Button>
            </>
          ) : (
            <Button size="sm" render={<Link to="/login" />}>
              {t('nav.login')}
            </Button>
          )}
        </div>
      </nav>
    </header>
  )
}
