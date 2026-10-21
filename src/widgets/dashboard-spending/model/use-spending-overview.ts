import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** Spending of a period against the previous one, the week and the sparkline's series. */
export function useSpendingOverview(period: MaybeRefOrGetter<DashPeriod>) {
  return useQuery({
    queryKey: computed(() => ['spending-overview', toValue(period)]),
    queryFn: () => call<SpendingOverview>('get_spending_overview', { period: toValue(period) }),
    placeholderData: keepPreviousData,
  })
}
