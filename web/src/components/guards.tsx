// Кто какую страницу видит. ВАЖНО: это только удобство — увести гостя на вход
// вместо пустого экрана. Настоящая проверка прав живёт на бэкенде, и обойти её
// правкой фронта нельзя.
import { useTranslation } from 'react-i18next'
import { Navigate } from 'react-router'
import { useAuth } from '@/auth/AuthContext'

/** Страница только для вошедших; пока сессия восстанавливается — ничего не мигает */
export function RequireAuth({ children }: { children: React.ReactNode }) {
  const { user, ready } = useAuth()
  const { t } = useTranslation()
  if (!ready) return <p className="p-8 text-sm text-muted-foreground">{t('common.loading')}</p>
  if (!user) return <Navigate to="/login" replace />
  return children
}
