// Вывод ошибок форм. Тексты приходят с бэка уже на языке пользователя:
// сервер — единственный авторитет валидации, дубликата правил на фронте нет.
import { ApiError, fieldError } from '@/api/client'

/**
 * Ошибка про форму целиком: сеть, неверная пара почта/пароль, сбой сервера.
 * Ошибку, привязанную к полю, здесь не показываем — она уйдёт под своё поле.
 */
export function FormError({ error }: { error: unknown }) {
  if (!error) return null
  if (error instanceof ApiError && error.fields.length > 0) return null
  const message = error instanceof Error ? error.message : String(error)
  return (
    <p role="alert" className="text-sm text-destructive">
      {message}
    </p>
  )
}

/** Ошибка конкретного поля — показывается прямо под ним */
export function FieldError({ error, field }: { error: unknown; field: string }) {
  const message = fieldError(error, field)
  if (!message) return null
  return (
    <p role="alert" className="text-sm text-destructive">
      {message}
    </p>
  )
}

/** Подсветить поле, к которому относится ошибка: `{...invalid(error, 'email')}` */
export function invalid(error: unknown, field: string) {
  return { 'aria-invalid': fieldError(error, field) !== undefined }
}
