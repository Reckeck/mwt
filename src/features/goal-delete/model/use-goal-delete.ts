import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'

/** Everything a goal shows up in: plan, envelopes, the automation it switched off, open cards. */
const STALE_KEYS: ReadonlyArray<readonly string[]> = [...PLANNING_KEYS, ['envelopes'], ['workflows'], ['entity-card']]

/** Delete a goal (`delete_goal`); the toast's «Отменить» puts it back (`restore_goal`). */
export function useGoalDelete() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const refresh = () => {
    for (const queryKey of STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
  }

  return useMutation({
    mutationFn: (id: string) => call<GoalDeleted>('delete_goal', { id }),
    onSuccess: refresh,
    meta: {
      error: 'goal.deleteFailed',
      success: (data) => t('goalDelete.deleted', { name: (data as GoalDeleted).goal.name }),
      undo: (data) => call<Goal>('restore_goal', { deleted: data as GoalDeleted }),
    },
  })
}
