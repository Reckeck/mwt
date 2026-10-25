import { computed } from 'vue'
import { useQueries } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'

import { moneyMoment } from '../lib/money-moment'
import { type Conversion, useConversions } from './conversions'

/**
 * `amount` of `from` in the base and favourite currencies at the rate of a
 * moment (`at`, RFC 3339 or a month `YYYY-MM`): the same rows as `useConversions().convert`, but a
 * row without a rate on that day is left out. Only used with a moment; for
 * now the cached table answers at once.
 */
export function useConversionsAt(amount: () => number, from: () => string, at: () => string) {
  const conversions = useConversions(true)
  const moment = () => moneyMoment(at())
  const codes = computed(() => (moment() ? conversions.convert(1, from()).map((row) => row.currency) : []))
  const results = useQueries({
    queries: computed(() =>
      codes.value.map((code) => ({
        queryKey: ['convert-amount', from(), code, amount(), moment()],
        queryFn: async () => {
          const input: ConvertAmountInput = { from: from(), to: code, amount: String(amount()), at: moment() || null }
          const result = await call<ConvertedAmount>('convert_amount', { input })
          return result.amount === null ? null : Number(result.amount)
        },
        staleTime: 60_000,
      })),
    ),
  })
  const rows = computed<Conversion[]>(() =>
    codes.value.flatMap((currency, index) => {
      const value = results.value[index]?.data
      return typeof value === 'number' ? [{ currency, value, text: formatMoney(value, { currency }), stale: '' }] : []
    }),
  )
  const pending = computed(() => conversions.table.isPending.value || results.value.some((entry) => entry.isPending))
  return { rows, pending }
}
