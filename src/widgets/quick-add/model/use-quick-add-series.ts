import { nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useCategories } from '@/shared/api/queries'
import { notify } from '@/shared/lib/notify'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { openTxDetails } from '@/shared/lib/tx-details'
import { txId } from '@/shared/lib/transactions'

import { savedText } from '../lib/saved-text'

const FORM_ID = 'transaction-form'

/**
 * Enter and Ctrl Enter in the short form (OpForm-1 · 4). Enter saves and
 * closes, the toast offers `Undo`. Ctrl Enter saves and leaves the form
 * open on the same wallet and date for the next one, counting the series; its
 * toast offers `Open` and `Undo`.
 */
export function useQuickAddSeries(close: () => void) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const categories = useCategories()

  /** Operations saved in this series; the title says the next one's number. */
  const series = ref(0)
  /** Whether the save under way came from Ctrl Enter. */
  let more = false

  const undo = useMutation({
    mutationFn: (id: string) => call('delete_transaction', { id }),
    onSuccess: () => {
      for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
    },
    meta: { success: 'toast.undone', error: 'tx.deleteFailed' },
  })

  /** Ctrl Enter: the same submit as the button, so the checks and errors are the same too. */
  function saveMore() {
    more = true
    const form = document.getElementById(FORM_ID)
    if (form instanceof HTMLFormElement) form.requestSubmit()
  }
  /** Enter or `Add`: a plain save that closes the form. */
  function saveOnce() {
    more = false
  }

  function saved(tx: Transaction) {
    const text = savedText(tx, categories.data.value, t)
    if (!more) {
      notify.success(text, { action: { label: t('actions.undo'), onClick: () => undo.mutate(txId(tx)) } })
      close()
      return
    }
    series.value += 1
    notify.success(text, {
      description: t('quickAdd.kept'),
      action: { label: t('actions.open'), onClick: () => openTxDetails(tx) },
      cancel: { label: t('actions.undo'), onClick: () => undo.mutate(txId(tx)) },
    })
    void nextTick(() => document.getElementById('tx-amount')?.focus())
  }

  function reset() {
    series.value = 0
    more = false
  }

  return { series, saveMore, saveOnce, saved, reset }
}
