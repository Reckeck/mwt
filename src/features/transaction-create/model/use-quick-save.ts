import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

/** Writing the quick form's operation; the caller says what happens after (the toast is the widget's). */
export function useQuickSave(onSaved: (tx: Transaction) => void) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: TransactionInput) => call<Transaction>('add_transaction', { input }),
    onSuccess: (tx) => {
      for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances'], ['contacts', 'summaries']])
        void queryClient.invalidateQueries({ queryKey })
      onSaved(tx)
    },
    // The widget says it itself, with `Undo`.
    meta: { success: () => '', error: 'form.saveFailed' },
  })
}
