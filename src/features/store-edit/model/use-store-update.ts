import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

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

export const STORE_STALE_KEYS = [['stores'], ['store'], ['summary'], ['net-worth'], ['budget'], ['daily-budget']]

/** One save path for every wallet settings form. */
export function useStoreUpdate() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: UpdateStoreInput) => call<Store>('update_store', { input }),
    onSuccess: () => {
      toast.success(t('stores.saved'))
      for (const queryKey of STORE_STALE_KEYS) void queryClient.invalidateQueries({ queryKey })
    },
    onError: (err) => toast.error(t('stores.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
  })
}
