import { computed, ref } from 'vue'
import { useMutation } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'

/**
 * This device waits to receive a vault: visible nearby by its name for five
 * minutes; the backend pushes every step (`transfer-receive`).
 */
export function useReceiveVault() {
  const view = ref<TransferReceiveView | null>(null)
  useBackendEvent<TransferReceiveView>('transfer-receive', (next) => {
    view.value = next
  })

  const start = useMutation({
    mutationFn: () => call<TransferReceiveView>('transfer_receive_start'),
    onSuccess: (next) => {
      view.value = next
    },
    meta: { silent: 'the dialog shows this device waiting', error: 'transfer.receive.startFailed' },
  })

  const decide = useMutation({
    mutationFn: (accept: boolean) => call('transfer_receive_decide', { accept }),
    meta: { silent: 'the code or the waiting screen follows', error: 'transfer.receive.decideFailed' },
  })

  function stop() {
    void call('transfer_receive_stop').catch(() => {})
    view.value = null
  }

  const phase = computed(() => view.value?.phase ?? 'idle')
  return { view, phase, start, decide, stop }
}
