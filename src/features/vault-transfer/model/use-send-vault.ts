import { computed, ref } from 'vue'
import { useMutation } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useBackendEvent } from '@/shared/api/use-backend-event'

/**
 * Hand this vault to a new device: the devices waiting nearby, the password
 * typed again, then the code from the other screen (`transfer-send`).
 */
export function useSendVault() {
  const nearby = ref<TransferNearby[]>([])
  const view = ref<TransferSendView | null>(null)
  useBackendEvent<TransferNearby[]>('transfer-nearby', (next) => {
    nearby.value = next
  })
  useBackendEvent<TransferSendView>('transfer-send', (next) => {
    view.value = next.phase === 'idle' ? null : next
  })

  async function browse() {
    nearby.value = await call<TransferNearby[]>('transfer_browse_start').catch(() => [])
  }

  function close() {
    void call('transfer_send_cancel').catch(() => {})
    void call('transfer_browse_stop').catch(() => {})
    view.value = null
    nearby.value = []
  }

  const send = useMutation({
    mutationFn: (input: TransferSendInput) => call<TransferSendView>('transfer_send', { input }),
    onSuccess: (next) => {
      view.value = next
    },
    // A wrong password is shown under the field by the form.
    meta: { silent: 'the dialog shows the next step or the error under the password' },
  })

  const sendCode = useMutation({
    mutationFn: (code: string) => call('transfer_send_code', { code }),
    meta: { silent: 'the dialog shows the transfer or the error under the code' },
  })

  const phase = computed(() => view.value?.phase ?? 'idle')
  return { nearby, view, phase, browse, close, send, sendCode }
}
