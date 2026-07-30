import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api } from '@/api/client'
import { useAuth } from '@/auth/AuthContext'
import { FieldError, FormError, invalid } from '@/components/form'
import { PendingButton } from '@/components/states'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

/** Новый адрес заработает только после перехода по ссылке из письма на него */
export function EmailSection() {
  const { t } = useTranslation()
  const { user } = useAuth()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      await api.post('/api/auth/email', { new_email: email, current_password: password })
      setEmail('')
      setPassword('')
      toast.info(t('settings.email.sent'))
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('settings.email.title')}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <p className="text-sm text-muted-foreground">
          {t('settings.email.current', { email: user?.email ?? '' })}
        </p>
        <form onSubmit={submit} className="space-y-3">
          <div className="space-y-1">
            <Input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder={t('settings.email.new')}
              autoComplete="email"
              required
              {...invalid(error, 'new_email')}
            />
            <FieldError error={error} field="new_email" />
          </div>
          <div className="space-y-1">
            <Input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t('settings.email.password')}
              autoComplete="current-password"
              required
              {...invalid(error, 'current_password')}
            />
            <FieldError error={error} field="current_password" />
          </div>
          <FormError error={error} />
          <PendingButton type="submit" pending={busy}>
            {t('settings.email.submit')}
          </PendingButton>
        </form>
      </CardContent>
    </Card>
  )
}
