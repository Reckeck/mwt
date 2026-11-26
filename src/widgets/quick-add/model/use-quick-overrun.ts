import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { decimalCmp } from '@/shared/lib/decimal'

import { type Overrun, matchingBudgets, overrunOf } from '../lib/overrun'

interface OverrunRequest {
  kind: string
  category: string
  storeId: string
  amount: string
  currency: string
}

/** The limits an expense would break (total, category, store), the amount converted to each budget's currency; a banner, never a block. */
export function useQuickOverrun(request: MaybeRefOrGetter<OverrunRequest>) {
  const enabled = computed(() => toValue(request).kind === 'payment')
  const statuses = useQuery({
    queryKey: ['budget-status'],
    queryFn: () => call<BudgetStatus[]>('get_budget_status'),
    enabled,
  })
  const broken = useQuery({
    queryKey: computed(() => {
      const { category, storeId, amount, currency } = toValue(request)
      return ['budget-overrun', category, storeId, amount, currency, statuses.data.value]
    }),
    placeholderData: keepPreviousData,
    queryFn: async () => {
      const { category, storeId, amount, currency } = toValue(request)
      if (!currency || decimalCmp(amount, '0') !== 1) return []
      const found: Overrun[] = []
      for (const status of matchingBudgets(statuses.data.value ?? [], category, storeId)) {
        const converted = await call<ConvertedAmount>('convert_amount', {
          input: { from: currency, to: status.budget.currency, amount, at: null },
        })
        const overrun = overrunOf(status, converted.amount)
        if (overrun) found.push(overrun)
      }
      return found
    },
    enabled: computed(() => enabled.value && Boolean(statuses.data.value?.length)),
  })
  return computed(() => (enabled.value ? (broken.data.value ?? []) : []))
}
