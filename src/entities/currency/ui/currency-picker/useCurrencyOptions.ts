import { type MaybeRefOrGetter, computed, toValue } from 'vue'

import { activeLocale } from '@/shared/lib/locale'
import { useSettingsStore } from '@/shared/store/settings'

import { useFavoriteCurrencies } from '../../model/favorites'
import type { CurrencyKind, CurrencyOption } from './currency-picker.types'

/** `Intl.DisplayNames` is costly to build; one per locale is enough. */
const namers = new Map<string, Intl.DisplayNames | null>()
function currencyName(code: string, locale: string): string {
  if (!namers.has(locale)) {
    try {
      namers.set(locale, new Intl.DisplayNames([locale], { type: 'currency' }))
    } catch {
      namers.set(locale, null)
    }
  }
  try {
    const name = namers.get(locale)?.of(code.toUpperCase()) ?? ''
    return name.toUpperCase() === code.toUpperCase() ? '' : name
  } catch {
    return ''
  }
}

/**
 * The catalog as picker rows: favourites and the base currency first, then fiat,
 * then crypto. Fiat rows carry a localized name, so «рубль» finds RUB.
 */
export function useCurrencyOptions(kinds: MaybeRefOrGetter<CurrencyKind[]>, extra: MaybeRefOrGetter<string[]>) {
  const settings = useSettingsStore()
  const favoriteCodes = useFavoriteCurrencies()

  const all = computed<CurrencyOption[]>(() => {
    const wanted = new Set(toValue(kinds))
    const order = favoriteCodes.withBase.value
    const favorites = new Set(order)
    const locale = activeLocale.value
    const seen = new Set<string>()
    const rows: CurrencyOption[] = []
    const push = (code: string, kind: CurrencyKind) => {
      const lower = code.toLowerCase()
      if (!lower || seen.has(lower)) return
      seen.add(lower)
      rows.push({
        code: lower,
        kind,
        name: kind === 'fiat' ? currencyName(lower, locale) : '',
        favorite: favorites.has(lower),
      })
    }
    const fiat = new Set(settings.fiatCurrencies)
    for (const code of toValue(extra)) push(code, fiat.has(code.toLowerCase()) ? 'fiat' : 'crypto')
    if (wanted.has('fiat')) for (const code of settings.fiatCurrencies) push(code, 'fiat')
    if (wanted.has('crypto')) for (const code of settings.cryptoCurrencies) push(code, 'crypto')
    // Favourites first, in the user's order (the base leads); the rest keep the catalog order.
    const rank = (row: CurrencyOption) => (row.favorite ? order.indexOf(row.code) : order.length)
    return rows.sort((left, right) => rank(left) - rank(right))
  })

  /** Exact code, then code prefix, then name or code substring. */
  function filter(query: string): CurrencyOption[] {
    const needle = query.trim().toLowerCase()
    if (!needle) return all.value
    const exact: CurrencyOption[] = []
    const prefix: CurrencyOption[] = []
    const rest: CurrencyOption[] = []
    for (const row of all.value) {
      if (row.code === needle) exact.push(row)
      else if (row.code.startsWith(needle)) prefix.push(row)
      else if (row.code.includes(needle) || row.name.toLowerCase().includes(needle)) rest.push(row)
    }
    return [...exact, ...prefix, ...rest]
  }

  return { all, filter }
}
