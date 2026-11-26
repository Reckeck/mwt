import { type Ref, computed } from 'vue'
import { useInfiniteQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** Rows per request: what one `Show more` adds. */
export const TX_PAGE = 100

/**
 * The operations list a page at a time from the backend: sending the whole
 * journal on every open and every price refresh froze the screen. The filter
 * is part of the key, so a new filter starts again from the first page; an
 * invalidation of `['transactions']` refetches only the pages already loaded.
 */
export function useTxPages(filter: Ref<TxFilter>) {
  const query = useInfiniteQuery({
    queryKey: computed(() => ['transactions', 'page', filter.value]),
    queryFn: ({ pageParam }) =>
      call<TxPage>('list_transactions_page', { filter: filter.value, offset: pageParam, limit: TX_PAGE }),
    initialPageParam: 0,
    getNextPageParam: (last, pages) => {
      const loaded = pages.reduce((sum, page) => sum + page.items.length, 0)
      return loaded < last.total ? loaded : undefined
    },
    placeholderData: (previous) => previous,
  })
  const rows = computed(() => query.data.value?.pages.flatMap((page) => page.items) ?? [])
  const total = computed(() => query.data.value?.pages.at(-1)?.total ?? 0)
  return { query, rows, total }
}
