import { computed, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

export const EXCHANGE_KEY_STATE_QUERY_KEY = 'exchange-key-state'

/**
 * Where the exchange key of a wallet stands on this device: none, stored but its rights unchecked,
 * read-only, or unusable until the vault is at level 3. A key lives only on the device that stored it,
 * so a second device answers `missing` for a wallet connected elsewhere.
 */
export function useExchangeKeyState(storeId: Ref<string>, enabled: Ref<boolean>) {
  const state = useQuery({
    // no placeholder: another exchange's key state
    queryKey: computed(() => [EXCHANGE_KEY_STATE_QUERY_KEY, storeId.value]),
    queryFn: () => call<ExchangeKeyState>('read_exchange_key_state', { input: { store_id: storeId.value } }),
    enabled,
  })
  return {
    state,
    levelRequired: computed(() => state.data.value === 'level_required'),
    notVerified: computed(() => state.data.value === 'not_verified'),
  }
}
