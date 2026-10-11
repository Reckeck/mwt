import { computed, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { fiatRef } from '@/shared/lib/asset-ref'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

/** Wallet kinds that hold money only: a top-up or an exchange into them is in a currency. */
const MONEY_ONLY: StoreKind[] = ['bank', 'cash']

/**
 * What an account side of a form starts with, so the asset is not picked twice:
 * - `main`: the first position of that wallet (its largest), or its bound asset;
 * - `currency`: for a bank or cash account, the currency money arrives in —
 *   the account's own currency, else its first fiat, else the base currency.
 * Both are `null` for a contact or free text.
 */
export function useAccountAsset(side: Ref<AccountRef | null>) {
  const settings = useSettingsStore()
  const storeId = computed(() => (side.value && 'store' in side.value ? side.value.store : null))

  const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
  const store = computed(
    () => flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === storeId.value)?.store ?? null,
  )
  const overview = useQuery({
    queryKey: computed(() => ['store', storeId.value, false]),
    queryFn: () => call<StoreOverview>('get_store_overview', { storeId: storeId.value, includeChildren: false }),
    enabled: computed(() => storeId.value !== null),
  })

  const bound = computed<AssetRef | null>(() => {
    const binding = store.value?.binding
    return binding && typeof binding === 'object' && 'asset' in binding ? binding.asset : null
  })
  const positions = computed(() => overview.data.value?.positions.map((position) => position.asset) ?? [])

  const main = computed<AssetRef | null>(() => bound.value ?? positions.value[0] ?? null)

  const moneyOnly = computed(() => (store.value ? MONEY_ONLY.includes(store.value.kind) : false))
  const currency = computed<AssetRef | null>(() => {
    if (!store.value) return null
    if (bound.value?.chain === 'fiat') return bound.value
    const fiat = positions.value.find((asset) => asset.chain === 'fiat')
    if (fiat) return fiat
    return moneyOnly.value ? fiatRef(settings.baseCurrency) : null
  })

  return { store, main, currency, moneyOnly }
}
