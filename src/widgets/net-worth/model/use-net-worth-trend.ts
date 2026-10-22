import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** Net worth now against the period's start, with a value per step for the sparkline. */
export function useNetWorthTrend(period: MaybeRefOrGetter<DashPeriod>) {
  return useQuery({
    // Under `net-worth`, so every ledger write that refreshes the figure refreshes the trend.
    queryKey: computed(() => ['net-worth', 'trend', toValue(period)]),
    queryFn: () => call<NetWorthTrend>('get_net_worth_trend', { period: toValue(period) }),
    placeholderData: keepPreviousData,
  })
}
