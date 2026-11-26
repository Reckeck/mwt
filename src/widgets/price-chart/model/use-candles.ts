import { type MaybeRefOrGetter, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** The price candles of a symbol (`get_candles`); a new symbol or step is a new key and keeps the old rows meanwhile. */
export function useCandles(symbol: MaybeRefOrGetter<string>, intervalMin: MaybeRefOrGetter<number>, limit = 120) {
  return useQuery({
    queryKey: ['crypto', 'candles', symbol, intervalMin, limit],
    queryFn: async () => {
      const input: GetCandlesInput = { symbol: toValue(symbol), interval_min: toValue(intervalMin), limit }
      return (await call<GetCandlesOutput>('get_candles', { input })).candles
    },
    placeholderData: keepPreviousData,
  })
}
