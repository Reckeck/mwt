import { createI18n } from 'vue-i18n'

import en from '@/shared/config/i18n/locales/en.json'
import ru from '@/shared/config/i18n/locales/ru.json'

export const LOCALE_STORAGE_KEY = 'trade-app:locale'

/** Concrete locales the UI ships translations for. */
export type ResolvedLocale = 'en' | 'ru'

/** A locale preference: `auto` follows the browser/OS language. */
export type LocalePreference = ResolvedLocale | 'auto'

/**
 * Read the persisted preference and collapse it to a concrete locale.
 * `auto` (or a missing/stale value) resolves from the browser language.
 */
export function detectLocalePreference(): LocalePreference {
  const saved = localStorage.getItem(LOCALE_STORAGE_KEY)
  return saved === 'ru' || saved === 'en' || saved === 'auto' ? saved : 'auto'
}

export function resolveLocale(pref: LocalePreference): ResolvedLocale {
  if (pref === 'ru' || pref === 'en') return pref
  return navigator.language.toLowerCase().startsWith('ru') ? 'ru' : 'en'
}

export const i18n = createI18n({
  legacy: false,
  locale: resolveLocale(detectLocalePreference()),
  fallbackLocale: 'en',
  messages: { en, ru },
})

/** Persist a preference so the next cold start honours it before the vault opens. */
export function persistLocale(pref: LocalePreference): void {
  localStorage.setItem(LOCALE_STORAGE_KEY, pref)
}
