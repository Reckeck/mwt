import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** Every field `update_store` takes, filled from the store so a form only patches its own part. */
export function updateInputOf(store: Store, patch: Partial<UpdateStoreInput> = {}): UpdateStoreInput {
  return {
    id: store.id,
    name: store.name,
    color: store.color,
    note: store.note,
    purpose: null,
    binding: null,
    liability: store.liability,
    contact_id: store.contact_id,
    custody: null,
    property: null,
    secured_by: store.secured_by,
    provider: null,
    ...patch,
  }
}

export const STORE_STALE_KEYS = [
  ['stores'],
  ['store'],
  ['summary'],
  ['net-worth'],
  ['budget'],
  ['daily-budget'],
  ['emergency-fund'],
]

/** One save path for every wallet settings form. */
export function useStoreUpdate() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateStoreInput) => call<Store>('update_store', { input }),
    onSuccess: () => {
      for (const queryKey of STORE_STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
    },
    meta: { error: 'stores.saveFailed', success: 'stores.saved' },
  })
}
