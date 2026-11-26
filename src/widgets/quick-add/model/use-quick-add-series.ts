import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useCategories } from '@/shared/api/queries'
import { notify } from '@/shared/lib/notify'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { txId } from '@/shared/lib/transactions'
import { openTxDetails } from '@/shared/lib/tx-details'

import { savedText } from '../lib/saved-text'

/**
 * What happens after a save. `Add` closes the form and the toast offers `Undo`; `Save and add
 * another` (Ctrl Enter) leaves the form open on the same wallet, category and date, counting
 * the series; its toast offers `Open` and `Undo`.
 */
export function useQuickAddSeries(handlers: { close: () => void; next: () => unknown }) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const categories = useCategories()

  /** Operations saved in this series; the title says the next one's number. */
  const series = ref(0)
  /** Whether the save under way asked for another one. */
  let another = false

  const undo = useMutation({
    mutationFn: (id: string) => call('delete_transaction', { id }),
    onSuccess: () => {
      for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
    },
    meta: { success: 'toast.undone', error: 'tx.deleteFailed' },
  })

  /** Say which button the coming save came from. */
  function intend(more: boolean) {
    another = more
  }

  function saved(tx: Transaction) {
    const text = savedText(tx, categories.data.value, t)
    const cancel = { label: t('actions.undo'), onClick: () => undo.mutate(txId(tx)) }
    if (!another) {
      notify.success(text, { action: cancel })
      handlers.close()
      return
    }
    series.value += 1
    notify.success(text, {
      description: t('quickAdd.kept'),
      action: { label: t('actions.open'), onClick: () => openTxDetails(tx) },
      cancel,
    })
    handlers.next()
  }

  function reset() {
    series.value = 0
    another = false
  }

  return { series, intend, saved, reset }
}
