import type { Ref } from 'vue'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/**
 * `Remove` on the exchange's import (Wallets-2 · 8): forgets the API key.
 * The wallet, its operations and what waits for confirmation stay; a new key
 * connects it again.
 */
export function useExchangeDisconnect(storeId: Ref<string>) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => call('clear_exchange_key', { input: { store_id: storeId.value } }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['bybit-key-status', storeId.value] }),
    meta: { error: 'wallet.bybit.keyRemoveFailed', success: 'wallet.bybit.keyRemoved' },
  })
}
