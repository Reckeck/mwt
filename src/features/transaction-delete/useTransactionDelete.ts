import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { call, parseError } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

/** Every query a deleted transaction can show up in. */
const STALE_KEYS: ReadonlyArray<readonly string[]> = [...LEDGER_KEYS, ...PLANNING_KEYS, ['contacts', 'balances']]

/** Delete one transaction by id and refresh everything derived from it. */
export function useTransactionDelete() {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => call('delete_transaction', { id }),
    onSuccess: () => {
      toast.success(t('tx.deleted'))
      for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
    },
    onError: (err) => toast.error(t('tx.deleteFailed'), { description: parseError(err).message }),
  })
}
