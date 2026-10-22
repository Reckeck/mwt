import { computed, toValue, type MaybeRefOrGetter } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** Under `report`, so every ledger write makes it stale with the other reports. */
export const MONTH_BALANCE_KEY = ['report', 'month-balance'] as const

/** What each month of the filter's period left, and what carried over. */
export function useMonthBalance(filter: MaybeRefOrGetter<ReportFilter>) {
  return useQuery({
    queryKey: computed(() => [...MONTH_BALANCE_KEY, JSON.parse(JSON.stringify(toValue(filter)))]),
    placeholderData: keepPreviousData,
    queryFn: () => call<MonthBalance>('get_month_balance', { filter: toValue(filter) }),
  })
}
