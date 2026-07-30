import { useTranslation } from 'react-i18next'
import { Link } from 'react-router'
import { Button } from '@/components/ui/button'

export default function NotFoundPage() {
  const { t } = useTranslation()
  return (
    <main className="mx-auto max-w-xl space-y-4 p-8 text-center">
      <h1 className="text-2xl font-bold">{t('notfound.title')}</h1>
      <Button render={<Link to="/" />}>{t('common.home')}</Button>
    </main>
  )
}
