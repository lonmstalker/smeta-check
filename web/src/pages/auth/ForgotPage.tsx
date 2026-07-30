import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { api } from '@/api/client'
import { FormError } from '@/components/form'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { AuthCard } from './AuthLayout'

export default function ForgotPage() {
  const { t } = useTranslation()
  const [email, setEmail] = useState('')

  const send = useMutation({
    mutationFn: () => api.post('/api/auth/forgot', { email }),
  })

  return (
    <AuthCard title={t('auth.forgot.title')}>
      {send.isSuccess ? (
        <p className="text-sm">{t('auth.forgot.sent')}</p>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault()
            send.mutate()
          }}
          className="space-y-4"
        >
          <p className="text-sm text-muted-foreground">{t('auth.forgot.hint')}</p>
          <Input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder={t('auth.email')}
            autoComplete="email"
            required
          />
          <FormError error={send.error} />
          <Button type="submit" disabled={send.isPending} className="w-full">
            {t('auth.forgot.submit')}
          </Button>
        </form>
      )}
    </AuthCard>
  )
}
