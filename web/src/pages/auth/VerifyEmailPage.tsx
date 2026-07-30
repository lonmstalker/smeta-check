// Открывается по ссылке из письма. Токен одноразовый, поэтому не тратим его
// на автозагрузке (StrictMode монтирует дважды) — подтверждение по кнопке.
import { useMutation } from '@tanstack/react-query'
import { useTranslation } from 'react-i18next'
import { Link, useSearchParams } from 'react-router'
import { api } from '@/api/client'
import { FormError } from '@/components/form'
import { Button } from '@/components/ui/button'
import { AuthCard } from './AuthLayout'

export default function VerifyEmailPage() {
  const { t } = useTranslation()
  const [params] = useSearchParams()
  const token = params.get('token') ?? ''

  const verify = useMutation({
    mutationFn: () => api.post('/api/auth/verify-email', { token }),
  })

  return (
    <AuthCard title={t('auth.verify.title')}>
      {verify.isSuccess ? (
        <div className="space-y-4">
          <p className="text-sm">{t('auth.verify.done')}</p>
          <Button className="w-full" render={<Link to="/" />}>
            {t('common.home')}
          </Button>
        </div>
      ) : (
        <div className="space-y-4">
          <FormError error={verify.error} />
          <Button className="w-full" disabled={verify.isPending} onClick={() => verify.mutate()}>
            {t('auth.verify.action')}
          </Button>
        </div>
      )}
    </AuthCard>
  )
}
