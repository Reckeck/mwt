import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

import { overrunOf } from '../lib/overrun'

/** The limit an expense would break, from the budgets of the month; a banner, never a block. */
export function useQuickOverrun(
  request: MaybeRefOrGetter<{ kind: string; category: string; amount: string; currency: string }>,
) {
  const statuses = useQuery({
    queryKey: ['budget-status'],
    queryFn: () => call<BudgetStatus[]>('get_budget_status'),
  })
  return computed(() => {
    const { kind, category, amount, currency } = toValue(request)
    if (kind !== 'payment' || !category || !currency) return null
    return overrunOf(statuses.data.value ?? [], category, amount, currency)
  })
}
