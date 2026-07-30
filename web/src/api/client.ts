// Типизированный клиент API. Типы приходят из OpenAPI (pnpm gen:api),
// поэтому фронт не может разойтись с бэком незаметно.
import type { components } from './schema'

export type User = components['schemas']['User']
export type TokenResponse = components['schemas']['TokenResponse']
export type LoginResponse = components['schemas']['LoginResponse']
export type Item = components['schemas']['Item']
export type ItemsPage = components['schemas']['ItemsPage']
export type TotpSetupResponse = components['schemas']['TotpSetupResponse']
export type FieldError = components['schemas']['FieldError']
export type SessionInfo = components['schemas']['SessionInfo']

// access-токен живёт только в памяти (не в localStorage — недоступен XSS);
// сессию между перезагрузками держит httpOnly refresh-cookie
let accessToken: string | null = null

export function setAccessToken(token: string | null) {
  accessToken = token
}

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    /// ошибки конкретных полей формы, если сервер их прислал
    public fields: FieldError[] = [],
  ) {
    super(message)
  }
}

/** Сообщение для поля формы: сервер — единственный источник правды о валидности */
export function fieldError(error: unknown, field: string): string | undefined {
  if (!(error instanceof ApiError)) return undefined
  return error.fields.find((f) => f.field === field)?.message
}

async function rawRequest<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: {
      ...(body !== undefined ? { 'content-type': 'application/json' } : {}),
      ...(accessToken ? { authorization: `Bearer ${accessToken}` } : {}),
    },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  })
  if (res.status === 204) return undefined as T
  const json = await res.json().catch(() => null)
  if (!res.ok) {
    throw new ApiError(
      res.status,
      json?.error?.code ?? 'error-internal',
      json?.error?.message ?? res.statusText,
      json?.error?.fields ?? [],
    )
  }
  return json as T
}

// сессия кончилась окончательно (refresh не помог) — интерфейсу пора
// перестать показывать пользователя вошедшим; ставит AuthContext
let onSessionExpired: (() => void) | null = null

export function setOnSessionExpired(handler: (() => void) | null) {
  onSessionExpired = handler
}

// один общий промис, чтобы параллельные 401 не устроили гонку refresh'ей
let refreshing: Promise<boolean> | null = null

async function tryRefresh(): Promise<boolean> {
  refreshing ??= rawRequest<TokenResponse>('POST', '/api/auth/refresh')
    .then((r) => {
      setAccessToken(r.access_token)
      return true
    })
    .catch(() => {
      setAccessToken(null)
      onSessionExpired?.()
      return false
    })
    .finally(() => {
      refreshing = null
    })
  return refreshing
}

// Ручки самого входа: их 401 значит «неверные данные», а не «протухла сессия» —
// refresh не поможет. Остальные /api/auth/* (сессии, пароль, почта) — обычные
// аккаунтные действия, им протухший токен обновляем как всем.
const NO_REFRESH = [
  '/api/auth/refresh',
  '/api/auth/login',
  '/api/auth/register',
  '/api/auth/2fa/verify',
]

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  try {
    return await rawRequest<T>(method, path, body)
  } catch (err) {
    // протухший access-токен обновляем незаметно для пользователя
    const canRetry = err instanceof ApiError && err.status === 401 && !NO_REFRESH.includes(path)
    if (canRetry && (await tryRefresh())) {
      return rawRequest<T>(method, path, body)
    }
    throw err
  }
}

export const api = {
  get: <T>(path: string) => request<T>('GET', path),
  post: <T>(path: string, body?: unknown) => request<T>('POST', path, body),
  patch: <T>(path: string, body?: unknown) => request<T>('PATCH', path, body),
  delete: <T>(path: string) => request<T>('DELETE', path),
}
