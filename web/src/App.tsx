import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useState } from 'react'
import { Route, Routes } from 'react-router'
import { Toaster } from 'sonner'
import { AuthProvider } from '@/auth/AuthContext'
import { AppShell } from '@/components/AppShell'
import { ErrorBoundary } from '@/components/ErrorBoundary'
import { RequireAuth } from '@/components/guards'
import ConfirmEmailPage from '@/pages/auth/ConfirmEmailPage'
import ForgotPage from '@/pages/auth/ForgotPage'
import LoginPage from '@/pages/auth/LoginPage'
import RegisterPage from '@/pages/auth/RegisterPage'
import ResetPage from '@/pages/auth/ResetPage'
import VerifyEmailPage from '@/pages/auth/VerifyEmailPage'
import ItemsPage from '@/pages/ItemsPage'
import NotFoundPage from '@/pages/NotFoundPage'
import SettingsPage from '@/pages/SettingsPage'

// свой клиент на экземпляр приложения (а не модульный синглтон), иначе кэш
// протекает между тестами
function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      // не дёргаем сеть лишний раз — данные считаются свежими 30 секунд
      queries: { staleTime: 30_000, retry: 1 },
    },
  })
}

export default function App() {
  const [queryClient] = useState(makeQueryClient)
  return (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>
        <ErrorBoundary>
          <AppShell>
            <Routes>
              <Route path="/" element={<ItemsPage />} />
              <Route path="/login" element={<LoginPage />} />
              <Route path="/register" element={<RegisterPage />} />
              <Route path="/forgot" element={<ForgotPage />} />
              <Route path="/reset" element={<ResetPage />} />
              <Route path="/verify-email" element={<VerifyEmailPage />} />
              <Route path="/confirm-email" element={<ConfirmEmailPage />} />
              <Route
                path="/settings"
                element={
                  <RequireAuth>
                    <SettingsPage />
                  </RequireAuth>
                }
              />
              <Route path="*" element={<NotFoundPage />} />
            </Routes>
          </AppShell>
          <Toaster position="top-center" />
        </ErrorBoundary>
      </AuthProvider>
    </QueryClientProvider>
  )
}
