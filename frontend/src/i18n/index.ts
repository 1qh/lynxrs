import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './locales/en.json'
import vi from './locales/vi.json'

const initial = (() => {
  try {
    const v = (globalThis as { localStorage?: Storage }).localStorage?.getItem('simu.lang')
    if (v === 'en' || v === 'vi') return v
  } catch {}
  return 'en'
})()

void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    vi: { translation: vi },
  },
  lng: initial,
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
})

export function setLang(l: 'en' | 'vi') {
  void i18n.changeLanguage(l)
  try {
    ;(globalThis as { localStorage?: Storage }).localStorage?.setItem('simu.lang', l)
  } catch {}
}

export default i18n
