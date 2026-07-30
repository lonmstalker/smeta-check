// Дата и время в привычном пользователю виде. С бэка приходит RFC 3339 в UTC:
// перевести в местный пояс и формат умеет сам браузер — своего форматирования
// (и библиотеки ради него) не заводим.
import { useTranslation } from 'react-i18next'

export function DateTime({ value }: { value: string }) {
  const { i18n } = useTranslation()
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return null
  const text = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date)
  return <time dateTime={value}>{text}</time>
}
