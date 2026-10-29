import { computed, toValue, type MaybeRefOrGetter } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/**
 * `Investments` for these wallets (`get_investments`). The key starts with
 * `report` so whatever refreshes the reports (a new operation, new rates) refreshes it too.
 */
export function useInvestments(storeIds: MaybeRefOrGetter<string[]>) {
  return useQuery({
    queryKey: computed(() => ['report', 'investments', [...toValue(storeIds)]]),
    // Another wallet filter keeps the old rows on screen until the new ones arrive.
    placeholderData: keepPreviousData,
    queryFn: () => call<Investments>('get_investments', { storeIds: toValue(storeIds) }),
  })
}
