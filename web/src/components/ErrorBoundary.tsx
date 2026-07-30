// Ошибка рендера не должна оставлять белый экран: показываем заглушку,
// подробности улетают в общий поток логов бэка через наш logger.
import { Component } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { log } from '@/lib/logger'

function Fallback() {
  const { t } = useTranslation()
  return (
    <main className="mx-auto max-w-xl space-y-4 p-8 text-center">
      <h1 className="text-2xl font-bold">{t('error.title')}</h1>
      <p className="text-sm text-muted-foreground">{t('error.hint')}</p>
      <Button onClick={() => window.location.assign('/')}>{t('common.home')}</Button>
    </main>
  )
}

export class ErrorBoundary extends Component<{ children: React.ReactNode }, { failed: boolean }> {
  state = { failed: false }

  static getDerivedStateFromError() {
    return { failed: true }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    log.error(`render crash: ${error.message}`, { componentStack: info.componentStack })
  }

  render() {
    return this.state.failed ? <Fallback /> : this.props.children
  }
}
