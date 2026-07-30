import { useMutation } from '@tanstack/react-query'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useSearchParams } from 'react-router'
import { api } from '@/api/client'
import { FormError } from '@/components/form'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { AuthCard } from './AuthLayout'

export default function ResetPage() {
  const { t } = useTranslation()
  const [params] = useSearchParams()
  const token = params.get('token') ?? ''
  const [password, setPassword] = useState('')

  const reset = useMutation({
    mutationFn: () => api.post('/api/auth/reset', { token, password }),
  })

  return (
    <AuthCard title={t('auth.reset.title')}>
      {reset.isSuccess ? (
        <div className="space-y-4">
          <p className="text-sm">{t('auth.reset.done')}</p>
          <Button className="w-full" render={<Link to="/login" />}>
            {t('auth.login.submit')}
          </Button>
        </div>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault()
            reset.mutate()
          }}
          className="space-y-4"
        >
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={t('auth.reset.password')}
            autoComplete="new-password"
            required
          />
          <FormError error={reset.error} />
          <Button type="submit" disabled={reset.isPending} className="w-full">
            {t('auth.reset.submit')}
          </Button>
        </form>
      )}
    </AuthCard>
  )
}
