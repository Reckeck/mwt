import { type Ref, watch } from 'vue'
import { useLocalStorage } from '@vueuse/core'

import { storeAncestorIds } from '@/shared/lib/store-tree'

/**
 * Which wallets show their sub-accounts, remembered between sessions; opening
 * a sub-account (from a link, a breadcrumb) unfolds its parents so it is seen.
 */
export function useExpandedStores(activeId: Ref<string>, roots: Ref<StoreTreeNode[]>) {
  // Stored as a list; a blocked storage just starts collapsed.
  const stored = useLocalStorage<string[]>('sidebar-expanded-stores', [])

  const isExpanded = (id: string) => stored.value.includes(id)

  watch(
    [activeId, roots],
    ([id, list]) => {
      if (!id || !list.length) return
      const parents = storeAncestorIds(list, id)
      if (parents.every(isExpanded)) return
      stored.value = [...new Set([...stored.value, ...parents])]
    },
    { immediate: true },
  )

  function toggle(id: string) {
    stored.value = isExpanded(id) ? stored.value.filter((entry) => entry !== id) : [...stored.value, id]
  }

  function collapseAll() {
    stored.value = []
  }

  return { isExpanded, toggle, collapseAll }
}
