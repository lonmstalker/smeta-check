// Локализация интерфейса. Новый язык = один json в src/locales + строка в
// resources; тест src/lib/i18n.test.ts проверит полноту ключей.
import i18n from 'i18next'
import LanguageDetector from 'i18next-browser-languagedetector'
import { initReactI18next } from 'react-i18next'
import en from '../locales/en.json'
import ru from '../locales/ru.json'

export const resources = {
  ru: { translation: ru },
  en: { translation: en },
} as const

void i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'ru',
    interpolation: { escapeValue: false }, // react сам экранирует
    detection: { order: ['localStorage', 'navigator'] },
  })

export default i18n
