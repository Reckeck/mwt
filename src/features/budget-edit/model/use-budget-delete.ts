import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'

/** What the toast names: the limit's own name («Продукты», «все траты»). */
export interface BudgetDeleteVars {
  budget: Budget
  name: string
}

/** Delete a limit (`delete_budget`); the toast's «Отменить» creates it again with every field. */
export function useBudgetDelete() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: ({ budget }: BudgetDeleteVars) => call('delete_budget', { id: budget.id }),
    onSuccess: () => {
      for (const queryKey of [...PLANNING_KEYS, ['notifications']]) void queryClient.invalidateQueries({ queryKey })
    },
    meta: {
      error: 'budget.deleteFailed',
      success: (_data, vars) => t('budget.deletedNamed', { name: (vars as BudgetDeleteVars).name }),
      undo: async (_data, vars) => {
        const { id: _id, ...input } = (vars as BudgetDeleteVars).budget
        await call<Budget>('create_budget', { input })
        for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
      },
    },
  })
}
