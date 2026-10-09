import { computed, type Ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/**
 * Whether a store has an exchange API key, and its masked hint.
 *
 * Shared by the key form and the page, which shows the import staging only
 * once a key exists; both read the same query.
 */
export function useExchangeKeyStatus(storeId: Ref<string>, enabled: Ref<boolean>) {
  const status = useQuery({
    queryKey: computed(() => ['bybit-key-status', storeId.value]),
    queryFn: () => call<ExchangeKeyStatus>('exchange_key_status', { input: { store_id: storeId.value } }),
    enabled,
  })
  return {
    status,
    hasKey: computed(() => status.data.value?.configured === true),
    hint: computed(() => status.data.value?.hint ?? ''),
  }
}
