import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { CRYPTO_OPERATIONS_KEY } from '@/entities/crypto-tx'
import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { notify } from '@/shared/lib/notify'

/** Everything a sync can change. */
export const EXCHANGE_SYNC_KEYS: ReadonlyArray<readonly unknown[]> = [
  CRYPTO_OPERATIONS_KEY,
  ['import-runs'],
  ['imported-transactions'],
  ['bybit-balances'],
  ...LEDGER_KEYS,
  ...PLANNING_KEYS,
]

/**
 * Syncing an exchange — "sync now" — saying how it went; shared by the wallet
 * page, its settings and the dashboard widget so they never disagree.
 *
 * Since spec E7 a record the exchange reports becomes an operation of the
 * crypto section straight away, so there is nothing to confirm or to discard
 * afterwards and no queue screen to show.
 */
export function useExchangeSync() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const refresh = () => Promise.all(EXCHANGE_SYNC_KEYS.map((queryKey) => queryClient.invalidateQueries({ queryKey })))

  const syncNow = useMutation({
    mutationFn: (storeId: string) =>
      call<ImportBybitHistoryOutput>('import_bybit_history', { input: { store_id: storeId } }),
    onSuccess: async (result) => {
      await refresh()
      if (result.notice) notify.warning(`import.notice.${result.notice}`)
    },
    meta: {
      error: 'import.notice.unreachable',
      // A notice (no key, nothing new) is a warning of its own, not a success.
      success: (data) => {
        const result = data as ImportBybitHistoryOutput
        if (result.notice) return ''
        const run = result.run
        return {
          text: t('import.synced'),
          description: t('cryptoTrack.legacy.syncedDetail', {
            recorded: run?.confirmed ?? 0,
            known: run?.known ?? 0,
          }),
        }
      },
    },
  })

  return { syncNow }
}
