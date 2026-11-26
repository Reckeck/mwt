import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import { goalInputOf } from '../lib/goal-fields'

/** Pause a goal (it leaves the plan and the pace figures) or resume it: `update_goal` with `active` flipped. */
export function useGoalPause() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (goal: Goal) =>
      call<Goal>('update_goal', {
        input: { id: goal.id, goal: { ...goalInputOf(goal), active: !goal.active }, base: goalInputOf(goal) },
      }),
    onSuccess: () => {
      for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    },
    meta: {
      error: 'goalCard.pauseFailed',
      success: (data) => t((data as Goal).active ? 'goalCard.resumed' : 'goalCard.paused'),
    },
  })
}
