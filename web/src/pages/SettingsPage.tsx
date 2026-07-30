// Настройки аккаунта. Страница только собирает разделы: каждый из них сам
// знает, что показывает и куда ходит.
import { useTranslation } from 'react-i18next'
import { Page, PageHeader } from '@/components/AppShell'
import { EmailSection } from './settings/EmailSection'
import { PasswordSection } from './settings/PasswordSection'
import { ProfileSection } from './settings/ProfileSection'
import { SessionsSection } from './settings/SessionsSection'
import { TotpSection } from './settings/TotpSection'

export default function SettingsPage() {
  const { t } = useTranslation()
  return (
    <Page>
      <PageHeader title={t('settings.title')} />
      <ProfileSection />
      <PasswordSection />
      <EmailSection />
      <TotpSection />
      <SessionsSection />
    </Page>
  )
}
