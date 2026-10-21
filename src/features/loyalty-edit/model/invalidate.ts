import type { QueryClient } from '@tanstack/vue-query'

import { LEDGER_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'

/** A programme or its points changed: the tree, the cashback report and the ledger read it. */
export function invalidateLoyalty(queryClient: QueryClient) {
  for (const queryKey of [STORE_TREE_QUERY_KEY, ['cashback'], ...LEDGER_KEYS]) {
    void queryClient.invalidateQueries({ queryKey })
  }
}
