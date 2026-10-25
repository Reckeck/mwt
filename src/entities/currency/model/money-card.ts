import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

import { useConversions } from './conversions'
import { useConversionsAt } from './conversions-at'
import { useFavoriteCurrencies } from './favorites'

/** `amount` of `from` in `to` at a moment (now when `at` is empty), or `null` without a rate. */
export function useConverted(
  amount: () => number,
  from: () => string,
  to: () => string,
  at: () => string,
  enabled: () => boolean,
) {
  return useQuery({
    queryKey: computed(() => ['convert-amount', from(), to(), amount(), at()]),
    queryFn: async () => {
      const input: ConvertAmountInput = { from: from(), to: to(), amount: String(amount()), at: at() || null }
      const result = await call<ConvertedAmount>('convert_amount', { input })
      return result.amount === null ? null : Number(result.amount)
    },
    enabled: computed(enabled),
    // no placeholder: the card is remounted for each sum, a previous sum's value would be wrong.
    staleTime: 60_000,
  })
}

/**
 * What the sum card (Currency-1 · 5) shows: the sum in the base currency at
 * the operation's rate and at today's, and in the favourites. Values are
 * positive: the card's title says which way the money went.
 */
export function useMoneyCard(
  amount: MaybeRefOrGetter<number>,
  currency: MaybeRefOrGetter<string>,
  at: MaybeRefOrGetter<string>,
) {
  const settings = useSettingsStore()
  const favorites = useFavoriteCurrencies()
  const conversions = useConversions(true)

  const size = () => Math.abs(toValue(amount))
  const from = () => toValue(currency).toLowerCase()
  const base = () => settings.baseCurrency
  const foreign = () => from() !== base()

  const then = useConverted(
    size,
    from,
    base,
    () => toValue(at),
    () => foreign() && !!toValue(at),
  )
  const now = useConverted(size, from, base, () => '', foreign)

  /** Units of the sum's currency per one of the base: «93,75 ₽ за $». */
  const rate = (value: number | null | undefined) => (value ? size() / value : null)

  // With a moment the favourites are at the operation's rate too, not at today's.
  const dated = useConversionsAt(size, from, () => toValue(at))
  const favoriteRows = computed(() =>
    (toValue(at) ? dated.rows.value : conversions.convert(size(), from())).filter(
      (row) => row.currency !== base() && favorites.isFavorite(row.currency),
    ),
  )

  return {
    base: computed(base),
    foreign: computed(foreign),
    atMoment: computed(() => then.data.value ?? null),
    now: computed(() => now.data.value ?? null),
    atMomentRate: computed(() => rate(then.data.value)),
    nowRate: computed(() => rate(now.data.value)),
    favorites: favoriteRows,
    pending: computed(() => then.isFetching.value || now.isFetching.value),
  }
}
