import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { txId, txMeta } from '@/shared/lib/transactions'

import { isSimilar, pairRate, rateDelta, similarFilter, txPair } from '../lib/tx-card'

/** How many past operations the card looks through. */
const LOOK_BACK = 50

/**
 * `Similar · 2` (Paths 1 · 6): earlier operations like this one, and how its
 * rate compares with theirs (`1.3 % higher than before`).
 */
export function useSimilarTxs(tx: MaybeRefOrGetter<Transaction | null>, enabled: MaybeRefOrGetter<boolean>) {
  const filter = computed(() => {
    const current = toValue(tx)
    return current ? similarFilter(current) : null
  })

  const page = useQuery({
    // no placeholder: another operation's similar ones
    queryKey: computed(() => ['transactions', 'similar', filter.value]),
    queryFn: () => call<TxPage>('list_transactions_page', { filter: filter.value, offset: 0, limit: LOOK_BACK }),
    enabled: computed(() => toValue(enabled) && Boolean(filter.value)),
  })

  const items = computed(() => {
    const current = toValue(tx)
    if (!current) return []
    const at = txMeta(current).timestamp
    return (page.data.value?.items ?? []).filter(
      (other) => txId(other) !== txId(current) && txMeta(other).timestamp < at && isSimilar(current, other),
    )
  })

  const delta = computed(() => {
    const current = toValue(tx)
    const pair = current ? txPair(current) : null
    if (!pair) return null
    const past = items.value.map((other) => txPair(other)).flatMap((other) => (other ? [pairRate(other)] : []))
    return rateDelta(pairRate(pair), past)
  })

  return { filter, items, delta, loading: computed(() => page.isPending.value && Boolean(filter.value)) }
}
