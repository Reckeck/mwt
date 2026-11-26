import { type Ref, computed } from 'vue'
import { keepPreviousData, useInfiniteQuery } from '@tanstack/vue-query'

import { cryptoOperationsListKey } from '@/entities/crypto-tx'
import { call } from '@/shared/api/ipc'

/** Rows one request brings: what one `Show more` adds. The backend caps it at the same number. */
export const CRYPTO_OPS_PAGE = 100

/**
 * The operations a page at a time. A history of ten thousand rows never travels or renders whole: the
 * backend filters and cuts the page, the list shows the pages loaded so far. The filter is in the key, so
 * a new filter starts from the first page, and `keepPreviousData` keeps the old rows (and the layout)
 * until the new ones arrive — nothing blanks or jumps while a filter changes (rule 17).
 */
export function useCryptoOps(filter: Ref<CryptoOpFilter>) {
  const query = useInfiniteQuery({
    queryKey: computed(() => cryptoOperationsListKey(filter.value)),
    queryFn: ({ pageParam }) =>
      call<CryptoOpsPage>('crypto_list_operations', {
        input: { filter: filter.value, offset: pageParam, limit: CRYPTO_OPS_PAGE },
      }),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((sum, page) => sum + page.items.length, 0)
      return loaded < last.total ? loaded : undefined
    },
    placeholderData: keepPreviousData,
  })
  const rows = computed(() => query.data.value?.pages.flatMap((page) => page.items) ?? [])
  const total = computed(() => query.data.value?.pages.at(-1)?.total ?? 0)
  const facets = computed(() => query.data.value?.pages[0]?.facets ?? { chains: [], protocols: [], tokens: [] })
  /** Only the first page carries it. */
  const suggestion = computed(() => query.data.value?.pages[0]?.suggestion)
  return { query, rows, total, facets, suggestion }
}
