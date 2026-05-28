import { createI18n } from 'vue-i18n'
import en from './locales/en.json'
import zh from './locales/zh.json'

const STORAGE_KEY = 'autoforge-language'

export type SupportedLocale = 'en' | 'zh'

function detectLocale(): SupportedLocale {
  const stored = localStorage.getItem(STORAGE_KEY) as SupportedLocale | null
  if (stored && ['en', 'zh'].includes(stored)) return stored
  const nav = navigator.language.toLowerCase()
  if (nav.startsWith('zh')) return 'zh'
  return 'en'
}

const i18n = createI18n({
  legacy: false,
  locale: detectLocale(),
  fallbackLocale: 'en',
  messages: { en, zh },
})

export function setLocale(locale: SupportedLocale) {
  localStorage.setItem(STORAGE_KEY, locale)
  i18n.global.locale.value = locale
  document.documentElement.setAttribute('lang', locale)
}

export function getLocale(): SupportedLocale {
  return i18n.global.locale.value as SupportedLocale
}

export function toggleLocale() {
  setLocale(getLocale() === 'en' ? 'zh' : 'en')
}

// Set initial lang attribute
document.documentElement.setAttribute('lang', getLocale())

export default i18n
