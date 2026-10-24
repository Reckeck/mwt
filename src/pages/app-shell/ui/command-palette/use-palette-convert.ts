import { type Ref, computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { useConversions, useFavoriteCurrencies } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { localDate } from '@/shared/lib/planning'
import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { txMeta, txPrincipalLegs } from '@/shared/lib/transactions'
import { useSettingsStore } from '@/shared/store/settings'

import { parseConvertQuery } from './convert-query'

/** As many rows as the frame shows: the asked one or the base, then the favourites. */
const NOW_ROWS = 3
/** How far back «на дату» looks for the last exchange. */
const EXCHANGE_LOOKBACK_DAYS = 365

export interface PaletteConversion {
  currency: string
  value: number
}

/**
 * Ctrl K as a converter (Currency-2 · 10): «100 usd», «5000 руб», «0.01 btc
 * в thb» read as an amount, shown at today's rate in the asked currency or
 * the base and the favourites, and at the rate of the day of the last
 * exchange in that currency.
 */
export function usePaletteConvert(open: Ref<boolean>, search: Ref<string>) {
  const settings = useSettingsStore()
  const favorites = useFavoriteCurrencies()
  const conversions = useConversions(open)

  const query = computed(() => (open.value ? parseConvertQuery(search.value, conversions.knows) : null))

  const now = computed<PaletteConversion[]>(() => {
    const asked = query.value
    if (!asked) return []
    const rows: PaletteConversion[] = []
    if (asked.to) {
      const value = conversions.convertTo(asked.amount, asked.from, asked.to)
      if (value !== null) rows.push({ currency: asked.to, value })
    }
    for (const row of conversions.convert(asked.amount, asked.from)) {
      if (row.currency === asked.to) continue
      if (row.currency === settings.baseCurrency || favorites.isFavorite(row.currency))
        rows.push({ currency: row.currency, value: row.value })
    }
    return rows.slice(0, NOW_ROWS)
  })
  /** The currencies Tab walks through: the base and the favourites, but the typed one. */
  const targets = computed(() => favorites.withBase.value.filter((code) => code !== query.value?.from))

  const from = computed(() => query.value?.from ?? '')
  const lastExchange = useQuery({
    queryKey: computed(() => ['palette-last-exchange', from.value]),
    queryFn: async () => {
      const since = new Date()
      since.setDate(since.getDate() - EXCHANGE_LOOKBACK_DAYS)
      const list = await call<Transaction[]>('list_transactions', {
        filter: emptyTxFilter({ kinds: ['convert'], from: localDate(since) }),
      })
      const moments = list
        .filter((tx) => txPrincipalLegs(tx).some((leg) => leg.asset.symbol.toLowerCase() === from.value))
        .map((tx) => txMeta(tx).timestamp)
      return moments.sort().at(-1) ?? null
    },
    enabled: computed(() => Boolean(from.value)),
    // no placeholder: another currency's exchange date would be wrong for this one.
    staleTime: 60_000,
  })

  /** The first row's currency on the day of the last exchange: «100 $ на 21 окт. = 9 375,00 ₽». */
  const datedTo = computed(() => now.value[0]?.currency ?? '')
  const dated = useQuery({
    queryKey: computed(() => [
      'convert-amount',
      from.value,
      datedTo.value,
      query.value?.amount,
      lastExchange.data.value,
    ]),
    queryFn: async () => {
      const at = lastExchange.data.value ?? null
      const input: ConvertAmountInput = { from: from.value, to: datedTo.value, amount: String(query.value?.amount), at }
      const result = await call<ConvertedAmount>('convert_amount', { input })
      return result.amount === null ? null : { at: at ?? '', currency: datedTo.value, value: Number(result.amount) }
    },
    enabled: computed(() => Boolean(query.value && datedTo.value && lastExchange.data.value)),
    // no placeholder: a previous query's sum on its own day would read as this one's.
    staleTime: 60_000,
  })

  return {
    query,
    now,
    targets,
    dated: computed(() => (query.value && lastExchange.data.value ? (dated.data.value ?? null) : null)),
  }
}
