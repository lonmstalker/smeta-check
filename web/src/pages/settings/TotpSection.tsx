import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { TotpSetupResponse } from '@/api/client'
import { api } from '@/api/client'
import { useAuth } from '@/auth/AuthContext'
import { FormError } from '@/components/form'
import { PendingButton } from '@/components/states'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

/** Второй фактор: включение по коду из приложения-аутентификатора */
export function TotpSection() {
  const { t } = useTranslation()
  const { user, refreshUser } = useAuth()
  const [setup, setSetup] = useState<TotpSetupResponse | null>(null)
  const [code, setCode] = useState('')
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  /// Общая обвязка запроса: одна попытка за раз, ошибка — под формой
  const run = async (action: () => Promise<void>) => {
    setError(null)
    setBusy(true)
    try {
      await action()
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  const start = () =>
    run(async () => setSetup(await api.post<TotpSetupResponse>('/api/auth/2fa/setup')))

  const confirm = (e: React.FormEvent) => {
    e.preventDefault()
    if (!setup) return
    void run(async () => {
      await api.post('/api/auth/2fa/enable', { secret: setup.secret, code })
      setSetup(null)
      setCode('')
      await refreshUser()
    })
  }

  const disable = (e: React.FormEvent) => {
    e.preventDefault()
    void run(async () => {
      await api.post('/api/auth/2fa/disable', { code })
      setCode('')
      await refreshUser()
    })
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          {t('settings.2fa.status', {
            status: user?.totp_enabled ? t('settings.2fa.enabled') : t('settings.2fa.disabled'),
          })}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {user?.totp_enabled ? (
          <form onSubmit={disable} className="space-y-4">
            <p className="text-sm text-muted-foreground">{t('settings.2fa.disable_hint')}</p>
            <Input
              value={code}
              onChange={(e) => setCode(e.target.value)}
              placeholder={t('auth.2fa.code')}
              inputMode="numeric"
            />
            <FormError error={error} />
            <PendingButton type="submit" variant="destructive" pending={busy}>
              {t('settings.2fa.disable')}
            </PendingButton>
          </form>
        ) : setup ? (
          <form onSubmit={confirm} className="space-y-4">
            <p className="text-sm text-muted-foreground">{t('settings.2fa.secret_hint')}</p>
            <code className="block break-all rounded bg-muted p-2 text-sm">{setup.secret}</code>
            <Button variant="outline" render={<a href={setup.otpauth_url} />}>
              {t('settings.2fa.open_app')}
            </Button>
            <Input
              value={code}
              onChange={(e) => setCode(e.target.value)}
              placeholder={t('auth.2fa.code')}
              inputMode="numeric"
            />
            <FormError error={error} />
            <PendingButton type="submit" pending={busy}>
              {t('settings.2fa.confirm')}
            </PendingButton>
          </form>
        ) : (
          <>
            <FormError error={error} />
            <PendingButton onClick={() => void start()} pending={busy}>
              {t('settings.2fa.start')}
            </PendingButton>
          </>
        )}
      </CardContent>
    </Card>
  )
}
