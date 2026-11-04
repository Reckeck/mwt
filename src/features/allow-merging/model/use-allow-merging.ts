import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

/** «Allow merging»: clears the «keep» mark, so the next sync may fold the record into its twin again. */
export function useAllowMerging() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (input: AllowMergingInput) => call<null>('allow_merging', { input }),
    onSuccess: () => {
      for (const queryKey of [...LEDGER_KEYS, ...PLANNING_KEYS]) void queryClient.invalidateQueries({ queryKey })
    },
    meta: { error: 'keep.allowFailed', success: 'keep.allowed' },
  })
}
