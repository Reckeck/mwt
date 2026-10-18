import { ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { TX_FILTER_QUERY_KEYS, emptyTxFilter, txFilterFromQuery, txFilterToQuery } from '@/shared/lib/tx-filter'

/**
 * The list's filter survives switching tabs: module state, alive as long
 * as the app is.
 */
export const sharedTxFilter = ref<TxFilter>(emptyTxFilter())

/**
 * The shared filter mirrored in the address: a link or Back brings the same
 * list, and a reload keeps it. The address wins when it carries a filter.
 */
export function useTxFilterInUrl() {
  const route = useRoute()
  const router = useRouter()
  const fromUrl = txFilterFromQuery(route.query)
  if (fromUrl) sharedTxFilter.value = fromUrl
  watch(
    sharedTxFilter,
    (filter) => {
      const query: Record<string, unknown> = { ...route.query }
      for (const key of TX_FILTER_QUERY_KEYS) delete query[key]
      for (const [key, value] of Object.entries(txFilterToQuery(filter))) if (value) query[key] = value
      void router.replace({ query: query as Record<string, string> })
    },
    { immediate: true, deep: true },
  )
  return sharedTxFilter
}
