import { computed, onScopeDispose, ref, type Ref } from 'vue'
import { useQueryClient } from '@tanstack/vue-query'

import { txId } from '@/shared/lib/transactions'

/**
 * The newest copy of an opened transaction: after an edit every list under
 * `['transactions', …]` refetches, and the card follows the cached copy
 * instead of the one it was opened with.
 */
/** Other lists live under `transactions` too (loss rows…): anything that is not a transaction has no id. */
function idOf(entry: unknown): string | null {
  try {
    return txId(entry as Transaction) ?? null
  } catch {
    return null
  }
}

export function useLatestTx(opened: Ref<Transaction | null>) {
  const queryClient = useQueryClient()
  const version = ref(0)
  const unsubscribe = queryClient.getQueryCache().subscribe((event) => {
    if (event.query.queryKey[0] === 'transactions') version.value += 1
  })
  onScopeDispose(unsubscribe)

  return computed(() => {
    void version.value
    const tx = opened.value
    if (!tx) return null
    const id = txId(tx)
    for (const [, data] of queryClient.getQueriesData<unknown>({ queryKey: ['transactions'] })) {
      if (!Array.isArray(data)) continue
      const fresh = (data as Transaction[]).find((entry) => idOf(entry) === id)
      if (fresh) return fresh
    }
    return tx
  })
}
