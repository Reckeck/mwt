import { type Ref, computed } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { keepPreviousFor } from '@/shared/lib/query'

/**
 * One wallet as its page shows it: the overview (with or without its
 * sub-accounts), realized P&L, and what is derived from them. Query keys are
 * `['store', id, includeChildren]` and `['store-pnl', …]` — prefix invalidation
 * of `['store']` refreshes every wallet page.
 */
export function useStoreOverview(storeId: Ref<string>, includeChildren: Ref<boolean>) {
  const overview = useQuery({
    queryKey: computed(() => ['store', storeId.value, includeChildren.value]),
    queryFn: () =>
      call<StoreOverview>('get_store_overview', { storeId: storeId.value, includeChildren: includeChildren.value }),
    placeholderData: (previous) => previous,
  })
  const pnl = useQuery({
    queryKey: computed(() => ['store-pnl', storeId.value, includeChildren.value]),
    placeholderData: keepPreviousFor(storeId),
    queryFn: () =>
      call<RealizedPnl>('get_store_pnl', { storeId: storeId.value, includeChildren: includeChildren.value }),
  })

  const base = computed(() => overview.data.value?.base_currency ?? '')
  const store = computed(() => overview.data.value?.store)
  const isArchived = computed(() => store.value?.archived === true)
  const children = computed(() => overview.data.value?.children ?? [])
  const hasChildren = computed(() => children.value.length > 0)
  /** With sub-accounts, positions are merged per asset across the whole subtree. */
  const positions = computed(() => {
    const data = overview.data.value
    if (!data) return []
    return hasChildren.value && includeChildren.value ? data.subtree_positions : data.positions
  })
  const ancestors = computed(() => overview.data.value?.ancestors ?? [])
  const parentPath = computed(() => [...ancestors.value.map((crumb) => crumb.name), store.value?.name ?? ''])
  const canAddChild = computed(() => overview.data.value?.can_add_child === true)
  /** Part of the subtree value that sits on the wallet itself. */
  const ownShare = computed(() => {
    const total = Number(overview.data.value?.subtree_base ?? 0)
    return total ? Number(overview.data.value?.own_base ?? 0) / total : 0
  })

  return {
    overview,
    pnl,
    base,
    store,
    isArchived,
    children,
    hasChildren,
    positions,
    ancestors,
    parentPath,
    canAddChild,
    ownShare,
  }
}
