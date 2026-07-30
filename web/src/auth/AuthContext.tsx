// Состояние входа. При загрузке страницы пробуем тихо восстановить сессию
// по httpOnly refresh-cookie; access-токен живёт только в памяти.
import { useQueryClient } from '@tanstack/react-query'
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import type { LoginResponse, TokenResponse, User } from '@/api/client'
import { api, setAccessToken, setOnSessionExpired } from '@/api/client'

type AuthState = {
  user: User | null
  /** false, пока не завершилась первая попытка восстановить сессию */
  ready: boolean
  register: (email: string, password: string) => Promise<void>
  /** возвращает pending-токен, если требуется второй шаг (2FA) */
  login: (email: string, password: string) => Promise<string | null>
  verify2fa: (pendingToken: string, code: string) => Promise<void>
  logout: () => Promise<void>
  refreshUser: () => Promise<void>
}

const AuthContext = createContext<AuthState | null>(null)

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [ready, setReady] = useState(false)
  const queryClient = useQueryClient()

  // Кэш запросов принадлежит конкретному пользователю: без сброса следующий
  // вошедший в этой же вкладке увидел бы чужие данные (ключи вроде ['items']
  // одинаковы для всех, а данные считаются свежими 30 секунд).
  const forget = useCallback(() => {
    setAccessToken(null)
    setUser(null)
    queryClient.clear()
  }, [queryClient])

  const applySession = useCallback((session: TokenResponse) => {
    setAccessToken(session.access_token)
    setUser(session.user)
  }, [])

  /** новый вход (в том числе вместо другого пользователя) — кэш не наш */
  const acceptSession = useCallback(
    (session: TokenResponse) => {
      queryClient.clear()
      applySession(session)
    },
    [applySession, queryClient],
  )

  // восстановление своей же сессии кэш не трогает: он и так наш
  useEffect(() => {
    api
      .post<TokenResponse>('/api/auth/refresh')
      .then(applySession)
      .catch(() => {}) // нет живой сессии — это нормальное состояние
      .finally(() => setReady(true))
  }, [applySession])

  // сессия отозвана на сервере (обновление не помогло) — выходим сами,
  // иначе шапка показывает вошедшего, а страницы сыплют ошибками
  useEffect(() => {
    setOnSessionExpired(forget)
    return () => setOnSessionExpired(null)
  }, [forget])

  const register = useCallback(
    async (email: string, password: string) => {
      acceptSession(await api.post<TokenResponse>('/api/auth/register', { email, password }))
    },
    [acceptSession],
  )

  const login = useCallback(
    async (email: string, password: string) => {
      const res = await api.post<LoginResponse>('/api/auth/login', { email, password })
      if (res.requires_2fa) return res.pending_token ?? null
      acceptSession(res as TokenResponse)
      return null
    },
    [acceptSession],
  )

  const verify2fa = useCallback(
    async (pendingToken: string, code: string) => {
      acceptSession(
        await api.post<TokenResponse>('/api/auth/2fa/verify', {
          pending_token: pendingToken,
          code,
        }),
      )
    },
    [acceptSession],
  )

  const logout = useCallback(async () => {
    // выход обязан сработать всегда: даже если сессия уже мертва и запрос
    // вернул ошибку, локальное состояние надо очистить
    await api.post('/api/auth/logout').catch(() => {})
    forget()
  }, [forget])

  const refreshUser = useCallback(async () => {
    setUser(await api.get<User>('/api/users/me'))
  }, [])

  const value = useMemo(
    () => ({ user, ready, register, login, verify2fa, logout, refreshUser }),
    [user, ready, register, login, verify2fa, logout, refreshUser],
  )
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used inside AuthProvider')
  return ctx
}
