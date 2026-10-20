import { type Ref, computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { keepPreviousFor } from '@/shared/lib/query'

/**
 * Money in and out of a wallet per month (`get_store_months`), oldest first.
 * The key starts with `store` so a wallet refresh refetches it too.
 */
export function useStoreMonths(storeId: Ref<string>, includeChildren: Ref<boolean>, months = 6) {
  return useQuery({
    queryKey: computed(() => ['store', storeId.value, includeChildren.value, 'months', months]),
    placeholderData: keepPreviousFor(storeId),
    queryFn: () =>
      call<StoreMonthRow[]>('get_store_months', {
        storeId: storeId.value,
        includeChildren: includeChildren.value,
        months,
      }),
  })
}
