import { type Ref, computed, ref, watch } from 'vue'

import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'

const LAST_WALLET_KEY = 'mwt:shared-purchase-wallet'

function remembered(): string {
  try {
    return localStorage.getItem(LAST_WALLET_KEY) ?? ''
  } catch {
    return ''
  }
}

/** The wallet a purchase of mine is paid from: the last one used, else the first, once the list is there. */
export function useLastWallet(open: Ref<boolean>, needed: Ref<boolean>) {
  const storeId = ref('')
  const tree = useStoreTree(computed(() => open.value && needed.value))
  const nodes = computed(() => (tree.data.value ?? []).filter((node) => !node.store.archived))
  const flat = computed(() => flattenStoreTree(nodes.value))

  watch(
    [open, flat],
    ([isOpen, list]) => {
      if (!isOpen || !needed.value || list.some((node) => node.store.id === storeId.value)) return
      storeId.value = list.find((node) => node.store.id === remembered())?.store.id ?? list[0]?.store.id ?? ''
    },
    { immediate: true },
  )

  function remember() {
    try {
      localStorage.setItem(LAST_WALLET_KEY, storeId.value)
    } catch {
      // Blocked storage: the next form starts on the first wallet.
    }
  }

  const name = computed(() => flat.value.find((node) => node.store.id === storeId.value)?.store.name ?? '')
  return { storeId, nodes, name, remember }
}
