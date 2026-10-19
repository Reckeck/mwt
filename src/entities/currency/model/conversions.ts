import { type MaybeRefOrGetter, computed, toValue, watch } from 'vue'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { useSettingsStore } from '@/shared/store/settings'

import { useFavoriteCurrencies } from './favorites'

/** FX table freshness for hover hints; rates move slowly enough. */
export const FX_TABLE_STALE_MS = 10 * 60 * 1000
/** Most rows a conversion list shows: the base, then favourites, then tracked. */
export const CONVERSIONS_MAX_ROWS = 8

/** One amount in another currency. */
export interface Conversion {
  currency: string
  value: number
  text: string
  /** The rate's date when it is older than today, else empty. */
  stale: string
}

/** Shared by every hover on screen: one network top-up of stale rates per session is enough. */
const topUp = { done: false }

/**
 * Units of `code` per one unit of the table's base, or `null` when unknown.
 * The base itself is 1.
 */
function perBase(table: FxTable, code: string): number | null {
  if (code === table.base) return 1
  const row = table.rows.find((entry) => entry.currency === code)
  const value = row ? Number(row.per_base) : Number.NaN
  return Number.isFinite(value) && value > 0 ? value : null
}

/**
 * `amount` of `from` in the base currency and the favourites (then the other
 * tracked currencies), leaving out `from` itself. Empty when `from` has no rate.
 */
export function convertAll(
  amount: number,
  from: string,
  table: FxTable,
  favorites: string[],
  today: string,
): Conversion[] {
  const source = perBase(table, from)
  if (source === null || !Number.isFinite(amount)) return []
  const inBase = amount / source
  const favs = favorites.map((code) => code.toLowerCase())
  const rank = (code: string) => (code === table.base ? -1 : favs.includes(code) ? favs.indexOf(code) : favs.length)
  const codes = [
    table.base,
    ...table.rows.filter((row) => favs.includes(row.currency) || row.tracked).map((row) => row.currency),
  ]
  return [...new Set(codes)]
    .filter((code) => code !== from)
    .sort((left, right) => rank(left) - rank(right) || left.localeCompare(right))
    .slice(0, CONVERSIONS_MAX_ROWS)
    .flatMap((code) => {
      const rate = perBase(table, code)
      if (rate === null) return []
      const row = table.rows.find((entry) => entry.currency === code)
      const value = inBase * rate
      return [
        {
          currency: code,
          value,
          text: formatMoney(value, { currency: code }),
          stale: row?.updated_at && row.updated_at.slice(0, 10) < today ? formatDate(row.updated_at) : '',
        },
      ]
    })
}

/**
 * The FX table for converting amounts on screen, loaded when `enabled`
 * (a hover, a chart on screen). Stale favourite and tracked rates are topped
 * up from the public sources once per session.
 */
export function useConversions(enabled: MaybeRefOrGetter<boolean> = true) {
  const settings = useSettingsStore()
  const favorites = useFavoriteCurrencies()
  const queryClient = useQueryClient()
  const today = localDate(new Date())
  const table = useQuery({
    queryKey: ['fx-table'],
    queryFn: () => call<FxTable>('get_fx_table'),
    enabled: computed(() => toValue(enabled)),
    staleTime: FX_TABLE_STALE_MS,
  })

  watch(
    () => table.data.value,
    (data) => {
      if (!data || topUp.done) return
      const wanted = [
        ...new Set([
          ...favorites.codes.value,
          ...data.rows.filter((row) => row.tracked).map((row) => row.currency),
          data.base,
        ]),
      ]
      // Coins are rated from the price cache (refreshed with the prices), never by the FX sources.
      const coins = new Set(settings.cryptoCurrencies)
      const stale = wanted.filter((currency) => {
        const row = data.rows.find((entry) => entry.currency === currency)
        if (row?.crypto || (!row && coins.has(currency))) return false
        return currency !== 'usd' && (!row || row.updated_at.slice(0, 10) < today)
      })
      topUp.done = true
      if (!stale.length) return
      void call<EnsureFxOutput>('ensure_fx', { input: { currencies: stale, date: null } satisfies EnsureFxInput })
        .then((result) => {
          if (result.points.length) void queryClient.invalidateQueries({ queryKey: ['fx-table'] })
        })
        .catch(() => {
          /* offline: the cached rates stay, labelled with their date */
        })
    },
    { immediate: true },
  )

  /** `amount` of `from` in the base and favourite currencies (empty until the table is there). */
  function convert(amount: number, from: string): Conversion[] {
    const data = table.data.value
    return data ? convertAll(amount, from.toLowerCase(), data, favorites.codes.value, today) : []
  }

  /** `amount` of `from` in `to` at the cached rate; `null` while a rate is missing. */
  function convertTo(amount: number, from: string, to: string): number | null {
    const data = table.data.value
    const source = from.toLowerCase()
    const target = to.toLowerCase()
    if (source === target) return amount
    if (!data) return null
    const fromRate = perBase(data, source)
    const toRate = perBase(data, target)
    return fromRate === null || toRate === null ? null : (amount / fromRate) * toRate
  }

  /** Whether `code` can be converted at all: the base or a currency (coins included) with a rate. */
  function knows(code: string): boolean {
    const lower = code.toLowerCase()
    const data = table.data.value
    if (lower === settings.baseCurrency) return true
    return Boolean(data && perBase(data, lower) !== null)
  }

  return { table, convert, convertTo, knows }
}
