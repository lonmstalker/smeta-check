// Переход по ссылке из письма на новый адрес — здесь смена почты и происходит.
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useSearchParams } from 'react-router'
import { api } from '@/api/client'
import { FormError } from '@/components/form'
import { PendingButton } from '@/components/states'
import { Button } from '@/components/ui/button'
import { AuthCard } from './AuthLayout'

export default function ConfirmEmailPage() {
  const { t } = useTranslation()
  const [params] = useSearchParams()
  const token = params.get('token') ?? ''
  const [done, setDone] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    setError(null)
    setBusy(true)
    try {
      await api.post('/api/auth/email/confirm', { token })
      setDone(true)
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  return (
    <AuthCard title={t('auth.confirm_email.title')}>
      {done ? (
        <>
          <p className="text-sm text-muted-foreground">{t('auth.confirm_email.done')}</p>
          <Button render={<Link to="/login" />}>{t('nav.login')}</Button>
        </>
      ) : (
        <>
          <FormError error={error} />
          <PendingButton onClick={submit} pending={busy} disabled={!token}>
            {t('auth.confirm_email.action')}
          </PendingButton>
        </>
      )}
    </AuthCard>
  )
}
