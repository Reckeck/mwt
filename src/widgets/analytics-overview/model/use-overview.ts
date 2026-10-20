import { computed, toValue, type MaybeRefOrGetter } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** The overview for a filter; under `['report']`, so every ledger change refreshes it. */
export function useOverview(filter: MaybeRefOrGetter<ReportFilter>) {
  return useQuery({
    queryKey: computed(() => ['report', 'overview', JSON.parse(JSON.stringify(toValue(filter)))]),
    placeholderData: keepPreviousData,
    queryFn: () => call<ReportOverview>('get_report_overview', { filter: toValue(filter) }),
  })
}
