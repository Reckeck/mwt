// check-ui-ignore invalidate-all: a reset replaces the whole document, so any screen's data may have changed
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { useSandboxOpen } from '@/features/sandbox-open'
import { textOf } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'

/** «Сбросить» (the same demo data again) and «Выйти» (close, erase, back to the unlock screen). */
export function useSandboxBar() {
  const vault = useVaultStore()
  const settings = useSettingsStore()
  const queryClient = useQueryClient()
  const reopen = useSandboxOpen()

  const reset = useMutation({
    mutationFn: async () => {
      const info = await vault.resetSandbox()
      // The demo vault's own settings come back with it.
      await settings.loadVault()
      return info
    },
    // Reset, not just marked stale: nothing of the old demo (or a half-read new one) stays on screen.
    onSettled: () => queryClient.resetQueries(),
    meta: { success: 'sandbox.resetDone', error: 'sandbox.resetFailed' },
  })

  const exit = useMutation({
    mutationFn: () => vault.closeSandbox(),
    // The demo data must not be shown to the next vault, and nothing may ask a locked vault.
    onSuccess: () => queryClient.removeQueries(),
    meta: {
      success: 'sandbox.closed',
      error: 'sandbox.closeFailed',
      action: () => ({ label: textOf('sandbox.reopen'), onClick: () => reopen.mutate() }),
    },
  })

  return { reset, exit }
}
