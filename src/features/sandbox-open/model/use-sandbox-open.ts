// check-ui-ignore invalidate-all: the sandbox replaces the whole document, so any screen's data may have changed
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useRouter } from 'vue-router'

import { useVaultStore } from '@/shared/store/vault'

/**
 * «Открыть песочницу»: the demo vault opens and the dashboard follows. There is
 * no toast on success, the new screen is the answer; a refusal (a real vault
 * is still open) shows the backend's `SANDBOX` reason.
 */
export function useSandboxOpen() {
  const vault = useVaultStore()
  const router = useRouter()
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: async () => {
      const info = await vault.openSandbox()
      await router.replace({ name: 'dashboard' })
      return info
    },
    onSettled: () => queryClient.invalidateQueries(),
    // An empty text: no success toast, the opened dashboard is the answer.
    meta: { success: () => '', error: 'sandbox.openFailed' },
  })
}
