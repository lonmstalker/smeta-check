import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useNavigate } from 'react-router'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { useAuth } from '@/auth/AuthContext'
import { FieldError, FormError, invalid } from '@/components/form'
import { PendingButton } from '@/components/states'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

/**
 * Смена пароля закрывает все сессии, включая текущую, — поэтому сразу после
 * успеха уводим на страницу входа, а не оставляем в мёртвом интерфейсе.
 */
export function PasswordSection() {
  const { t } = useTranslation()
  const { logout } = useAuth()
  const navigate = useNavigate()
  const [current, setCurrent] = useState('')
  const [next, setNext] = useState('')
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      await api.post('/api/auth/password', {
        current_password: current,
        new_password: next,
      })
      toast.success(t('settings.password.done'))
      await logout()
      navigate('/login')
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('settings.password.title')}</CardTitle>
      </CardHeader>
      <CardContent>
        <form onSubmit={submit} className="space-y-3">
          <div className="space-y-1">
            <Input
              type="password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
              placeholder={t('settings.password.current')}
              autoComplete="current-password"
              required
              {...invalid(error, 'current_password')}
            />
            <FieldError error={error} field="current_password" />
          </div>
          <div className="space-y-1">
            <Input
              type="password"
              value={next}
              onChange={(e) => setNext(e.target.value)}
              placeholder={t('settings.password.new')}
              autoComplete="new-password"
              required
              {...invalid(error, 'new_password')}
            />
            <FieldError error={error} field="new_password" />
          </div>
          <FormError error={error} />
          <PendingButton type="submit" pending={busy}>
            {t('settings.password.submit')}
          </PendingButton>
        </form>
      </CardContent>
    </Card>
  )
}
