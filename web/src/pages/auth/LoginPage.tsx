import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useLocation, useNavigate } from 'react-router'
import { useAuth } from '@/auth/AuthContext'
import { FormError } from '@/components/form'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { AuthCard } from './AuthLayout'

export default function LoginPage() {
  const { t } = useTranslation()
  const { login, verify2fa } = useAuth()
  const navigate = useNavigate()
  const { hash } = useLocation()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [code, setCode] = useState('')
  // вход через провайдера с включённой 2FA возвращает сюда pending-токен
  // во фрагменте адреса — второй шаг ровно тот же, что и при обычном входе
  const [pendingToken, setPendingToken] = useState<string | null>(() =>
    new URLSearchParams(hash.slice(1)).get('pending'),
  )
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      const pending = await login(email, password)
      if (pending) setPendingToken(pending)
      else navigate('/')
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  const submitCode = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!pendingToken) return
    setError(null)
    setBusy(true)
    try {
      await verify2fa(pendingToken, code)
      navigate('/')
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  if (pendingToken) {
    return (
      <AuthCard title={t('auth.2fa.title')}>
        <p className="text-sm text-muted-foreground">{t('auth.2fa.hint')}</p>
        <form onSubmit={submitCode} className="space-y-4">
          <Input
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder={t('auth.2fa.code')}
            autoComplete="one-time-code"
            inputMode="numeric"
            autoFocus
          />
          <FormError error={error} />
          <Button type="submit" disabled={busy} className="w-full">
            {t('auth.2fa.submit')}
          </Button>
        </form>
      </AuthCard>
    )
  }

  return (
    <AuthCard title={t('auth.login.title')}>
      <form onSubmit={submit} className="space-y-4">
        <Input
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          placeholder={t('auth.email')}
          autoComplete="email"
          required
        />
        <Input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder={t('auth.password')}
          autoComplete="current-password"
          required
        />
        <FormError error={error} />
        <Button type="submit" disabled={busy} className="w-full">
          {t('auth.login.submit')}
        </Button>
      </form>
      <div className="flex flex-col gap-2">
        <Button variant="outline" render={<a href="/api/auth/oauth/vk/start" />}>
          {t('auth.login.vk')}
        </Button>
        <Button variant="outline" render={<a href="/api/auth/oauth/yandex/start" />}>
          {t('auth.login.yandex')}
        </Button>
      </div>
      <p className="text-sm text-muted-foreground">
        {t('auth.login.no_account')}{' '}
        <Link className="underline" to="/register">
          {t('auth.login.register_link')}
        </Link>
        {' · '}
        <Link className="underline" to="/forgot">
          {t('auth.login.forgot_link')}
        </Link>
      </p>
    </AuthCard>
  )
}
