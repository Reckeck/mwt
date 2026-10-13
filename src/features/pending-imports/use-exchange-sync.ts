import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { call, parseError } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

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
      if (result.notice) {
        toast.warning(t(`import.notice.${result.notice}`))
        return
      }
      const run = result.run
      toast.success(t('import.synced'), {
        description: t('import.syncedDetail', {
          staged: run?.staged ?? result.staged,
          known: run?.known ?? 0,
          confirmed: run?.confirmed ?? 0,
        }),
      })
    },
    onError: (err) => toast.error(t('import.notice.unreachable'), { description: parseError(err).message }),
  })

  const confirm = useMutation({
    mutationFn: (execIds: string[]) =>
      call<ConfirmPendingImportsOutput>('confirm_pending_imports', { input: { exec_ids: execIds } }),
    onSuccess: async (result) => {
      await refresh()
      toast.success(t('import.confirmed'), { description: String(result.confirmed) })
    },
    onError: (err) => toast.error(t('import.confirmFailed'), { description: parseError(err).message }),
  })

  const discard = useMutation({
    mutationFn: (execIds: string[]) =>
      call<DiscardPendingImportsOutput>('discard_pending_imports', { input: { exec_ids: execIds } }),
    onSuccess: async (result) => {
      await refresh()
      toast.success(t('import.discarded'), { description: String(result.discarded) })
    },
    onError: (err) => toast.error(t('import.discardFailed'), { description: parseError(err).message }),
  })

  return { syncNow, confirm, discard }
}
