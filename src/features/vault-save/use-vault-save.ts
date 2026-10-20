import { useEventListener } from '@vueuse/core'

import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'
import { notify, textOf } from '@/shared/lib/notify'
import { useVaultStore } from '@/shared/store'

/** `vault-save-failed`: the reason, or the reason and whether the app was quitting. */
type SaveFailed = string | { reason: string; exiting: boolean }

/**
 * Changes are written in batches (a moment after the last edit). This keeps
 * the batch safe: the window going to the background writes it at once (a
 * phone may kill the app there), and a write that fails says so and offers to
 * retry — on quitting, also to quit without the last changes.
 */
export function useVaultSave() {
  const vault = useVaultStore()

  const flush = () => call('flush_vault')

  useEventListener(document, 'visibilitychange', () => {
    if (document.visibilityState === 'hidden' && !vault.locked) flush().catch(() => undefined)
  })

  useBackendEvent<SaveFailed>('vault-save-failed', (payload) => {
    const reason = typeof payload === 'string' ? payload : payload.reason
    const exiting = typeof payload === 'object' && payload.exiting
    notify.error(reason, exiting ? 'vaultSave.failedOnExit' : 'vaultSave.failed', {
      sticky: true,
      action: {
        label: textOf('vaultSave.retry'),
        onClick: () => {
          flush()
            .then(() => notify.success('vaultSave.saved'))
            .catch((err: unknown) => notify.error(err, 'vaultSave.failed'))
        },
      },
      cancel: exiting
        ? { label: textOf('vaultSave.quitAnyway'), onClick: () => void call('exit_without_saving') }
        : undefined,
    })
  })

  useBackendEvent('vault-saved', () => notify.success('vaultSave.saved'))
}
