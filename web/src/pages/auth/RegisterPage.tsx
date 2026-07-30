import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Link, useNavigate } from 'react-router'
import { toast } from 'sonner'
import { useAuth } from '@/auth/AuthContext'
import { FieldError, FormError, invalid } from '@/components/form'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { AuthCard } from './AuthLayout'

export default function RegisterPage() {
  const { t } = useTranslation()
  const { register } = useAuth()
  const navigate = useNavigate()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    setError(null)
    setBusy(true)
    try {
      await register(email, password)
      toast.info(t('auth.register.verify_sent'))
      navigate('/')
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  return (
    <AuthCard title={t('auth.register.title')}>
      <form onSubmit={submit} className="space-y-4">
        <div className="space-y-1">
          <Input
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder={t('auth.email')}
            autoComplete="email"
            required
            {...invalid(error, 'email')}
          />
          <FieldError error={error} field="email" />
        </div>
        <div className="space-y-1">
          <Input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={t('auth.password')}
            autoComplete="new-password"
            required
            {...invalid(error, 'password')}
          />
          <FieldError error={error} field="password" />
        </div>
        <FormError error={error} />
        <Button type="submit" disabled={busy} className="w-full">
          {t('auth.register.submit')}
        </Button>
      </form>
      <p className="text-sm text-muted-foreground">
        {t('auth.register.have_account')}{' '}
        <Link className="underline" to="/login">
          {t('auth.login.submit')}
        </Link>
      </p>
    </AuthCard>
  )
}
