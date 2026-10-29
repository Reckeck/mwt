import { computed, type MaybeRefOrGetter, toValue } from 'vue'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { emptyTxFilter } from '@/shared/lib/tx-filter'

import { frequentCategory } from '../lib/frequent-category'

/** How far back `Often` looks: a quarter is enough to know the habit and stays cheap. */
const LOOKBACK_DAYS = 90

/** The category this kind of operation gets most often lately, offered under the field. */
export function useFrequentCategory(kind: MaybeRefOrGetter<TxKind>) {
  const since = new Date(Date.now() - LOOKBACK_DAYS * 86_400_000).toISOString().slice(0, 10)
  const recent = useQuery({
    queryKey: computed(() => ['transactions', 'frequent-category', toValue(kind), since]),
    queryFn: () =>
      call<Transaction[]>('list_transactions', { filter: emptyTxFilter({ kinds: [toValue(kind)], from: since }) }),
    staleTime: 60_000,
    placeholderData: keepPreviousData,
  })
  return computed(() => frequentCategory(recent.data.value ?? [], toValue(kind)))
}
