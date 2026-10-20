import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/**
 * Everything a fresh quote or rate changes. Refreshed by the shell on
 * `prices-updated` (whoever started it: this button, the tray, the timer).
 */
export const PRICED_KEYS = [['store'], ['store-history'], ['stores'], ['summary'], ['fx-table'], ['net-worth']]

/**
 * Fetch new quotes and exchange rates for everything held — one action from
 * the wallet page, the rates widget and the tray alike; the toast says how
 * many prices and rates came in.
 */
export function usePriceRefresh() {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: () => call<RefreshResult>('refresh_all'),
    // The screens follow `prices-updated` (use-cache-events), not this result.
    onSuccess: (result) => queryClient.setQueryData(['refresh'], result),
    meta: {
      error: 'wallet.pricesRefreshFailed',
      success: (data) => {
        const result = data as RefreshResult
        return { text: t('wallet.pricesRefreshed'), description: `${result.prices} · ${result.fx}` }
      },
    },
  })
}
