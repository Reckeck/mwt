import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { storeTxFilter } from '@/shared/lib/tx-filter'

/** `Latest operations`: three, newest first. */
const LAST = 3

/**
 * What a wallet's name opens (Paths 1 · 4): its value, the sub-accounts and the
 * last operations. The overview is the wallet page's query, so opening the page
 * after the glance reads the cache.
 */
export function useWalletGlance(storeId: MaybeRefOrGetter<string>, enabled: MaybeRefOrGetter<boolean>) {
  const on = computed(() => toValue(enabled) && Boolean(toValue(storeId)))
  const overview = useQuery({
    // no placeholder: another wallet
    queryKey: computed(() => ['store', toValue(storeId), true]),
    enabled: on,
    queryFn: () => call<StoreOverview>('get_store_overview', { storeId: toValue(storeId), includeChildren: true }),
  })
  const recent = useQuery({
    // no placeholder: another wallet
    queryKey: computed(() => ['transactions', 'glance-wallet', toValue(storeId)]),
    enabled: on,
    queryFn: () =>
      call<TxPage>('list_transactions_page', { filter: storeTxFilter(toValue(storeId)), offset: 0, limit: LAST }),
  })

  const data = computed(() => overview.data.value)
  return {
    store: computed(() => data.value?.store ?? null),
    currency: computed(() => data.value?.base_currency ?? ''),
    total: computed(() => data.value?.subtree_base ?? '0'),
    /** «BTC, USDT»: what the wallet holds, biggest first. */
    assets: computed(() =>
      [...(data.value?.subtree_positions ?? [])]
        .filter((position) => Number(position.quantity) !== 0)
        .map((position) => position.asset.symbol.toUpperCase())
        .filter((symbol, index, all) => all.indexOf(symbol) === index)
        .slice(0, 4),
    ),
    children: computed(() => data.value?.children ?? []),
    recent: computed(() => recent.data.value?.items ?? []),
    loading: computed(() => overview.isPending.value),
  }
}
