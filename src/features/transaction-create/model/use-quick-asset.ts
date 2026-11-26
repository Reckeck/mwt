import { type MaybeRefOrGetter, computed, toValue } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { fiatRef } from '@/shared/lib/asset-ref'
import { useSettingsStore } from '@/shared/store/settings'

/**
 * The asset a quick operation is written in: the one the wallet is bound to, else its largest
 * position, else the base currency (an empty bank or cash wallet). `null` while it is not known.
 */
export function useQuickAsset(storeId: MaybeRefOrGetter<string>) {
  const settings = useSettingsStore()
  const overview = useQuery({
    // no placeholder: the asset is this wallet's own, not the last one's
    queryKey: computed(() => ['store', toValue(storeId), false]),
    queryFn: () => call<StoreOverview>('get_store_overview', { storeId: toValue(storeId), includeChildren: false }),
    enabled: computed(() => toValue(storeId) !== ''),
  })

  return computed<AssetRef | null>(() => {
    if (!overview.isSuccess.value) return null
    const binding = overview.data.value?.store.binding
    if (binding && typeof binding === 'object' && 'asset' in binding) return binding.asset
    const held = overview.data.value?.positions[0]?.asset
    if (held) return held
    return settings.appLoaded ? fiatRef(settings.baseCurrency) : null
  })
}
