import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import type { User } from '@/api/client'
import { api } from '@/api/client'
import { useAuth } from '@/auth/AuthContext'
import { FieldError, FormError, invalid } from '@/components/form'
import { PendingButton } from '@/components/states'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'

/** Как обращаться к пользователю. Язык переключается кнопкой в шапке. */
export function ProfileSection() {
  const { t } = useTranslation()
  const { user, refreshUser } = useAuth()
  const [name, setName] = useState(user?.display_name ?? '')
  const [error, setError] = useState<unknown>(null)
  const [busy, setBusy] = useState(false)

  const save = async (patch: { display_name?: string }) => {
    setError(null)
    setBusy(true)
    try {
      await api.patch<User>('/api/users/me', patch)
      await refreshUser()
      toast.success(t('settings.profile.saved'))
    } catch (err) {
      setError(err)
    } finally {
      setBusy(false)
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('settings.profile.title')}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <form
          onSubmit={(e) => {
            e.preventDefault()
            void save({ display_name: name })
          }}
          className="space-y-3"
        >
          <div className="space-y-1">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('settings.profile.name')}
              aria-label={t('settings.profile.name')}
              {...invalid(error, 'display_name')}
            />
            <FieldError error={error} field="display_name" />
          </div>
          <FormError error={error} />
          <PendingButton type="submit" pending={busy}>
            {t('settings.profile.save')}
          </PendingButton>
        </form>
      </CardContent>
    </Card>
  )
}
