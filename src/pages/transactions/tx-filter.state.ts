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
  // A link to another filter while the list is open (a chip, a notice) comes in
  // through the address too. Only the filter's own keys are watched: opening a
  // card (`?open=`) on top of the list must not reset it.
  const filterKeys = () => JSON.stringify(TX_FILTER_QUERY_KEYS.map((key) => route.query[key] ?? null))
  watch(filterKeys, () => {
    const next = txFilterFromQuery(route.query) ?? emptyTxFilter()
    if (JSON.stringify(txFilterToQuery(next)) !== JSON.stringify(txFilterToQuery(sharedTxFilter.value))) {
      sharedTxFilter.value = next
    }
  })
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
