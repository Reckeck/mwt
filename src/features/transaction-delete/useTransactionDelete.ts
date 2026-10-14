import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

/** Every query a deleted transaction can show up in. */
const STALE_KEYS: ReadonlyArray<readonly string[]> = [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances']]

/** Delete one transaction by id and refresh everything derived from it. */
export function useTransactionDelete() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => call('delete_transaction', { id }),
    onSuccess: () => {
      for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
    },
    meta: { error: 'tx.deleteFailed', success: 'tx.deleted' },
  })
}
