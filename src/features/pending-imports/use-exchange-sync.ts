import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { notify } from '@/shared/lib/notify'

/** Everything a sync, a confirmation or a discard can change. */
export const EXCHANGE_SYNC_KEYS: ReadonlyArray<readonly unknown[]> = [
  ['pending-imports'],
  ['import-runs'],
  ['imported-transactions'],
  ['bybit-balances'],
  ...LEDGER_KEYS,
  ...PLANNING_KEYS,
]

/**
 * What the user does with an exchange's imports — sync now, confirm, discard —
 * each saying how it went; shared by the wallet page, its settings and the
 * dashboard widget so they never disagree.
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
          description: t('import.syncedDetail', {
            staged: run?.staged ?? result.staged,
            known: run?.known ?? 0,
            confirmed: run?.confirmed ?? 0,
          }),
        }
      },
    },
  })

  const confirm = useMutation({
    mutationFn: (execIds: string[]) =>
      call<ConfirmPendingImportsOutput>('confirm_pending_imports', { input: { exec_ids: execIds } }),
    onSuccess: refresh,
    meta: {
      error: 'import.confirmFailed',
      success: (data) => ({
        text: t('import.confirmed'),
        description: String((data as ConfirmPendingImportsOutput).confirmed),
      }),
    },
  })

  const discard = useMutation({
    mutationFn: (execIds: string[]) =>
      call<DiscardPendingImportsOutput>('discard_pending_imports', { input: { exec_ids: execIds } }),
    onSuccess: refresh,
    meta: {
      error: 'import.discardFailed',
      success: (data) => ({
        text: t('import.discarded'),
        description: String((data as DiscardPendingImportsOutput).discarded),
      }),
    },
  })

  return { syncNow, confirm, discard }
}
