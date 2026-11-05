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
  // Bumped by close(): a send that was still checking the password when the dialog
  // closed finishes later and must not bring the view back.
  let epoch = 0
  // Events of a send are ignored while the dialog is closed (browse() opens, close() closes).
  let active = false
  useBackendEvent<TransferSendView>('transfer-send', (next) => {
    if (!active) return
    view.value = next.phase === 'idle' ? null : next
  })

  async function browse() {
    active = true
    nearby.value = await call<TransferNearby[]>('transfer_browse_start').catch(() => [])
  }

  function close() {
    epoch += 1
    active = false
    void call('transfer_send_cancel').catch(() => {})
    void call('transfer_browse_stop').catch(() => {})
    view.value = null
    nearby.value = []
  }

  const send = useMutation({
    mutationFn: (input: TransferSendInput) => call<TransferSendView>('transfer_send', { input }),
    onMutate: () => ({ startedAt: epoch }),
    onSuccess: (next, _input, context) => {
      if (context?.startedAt !== epoch) {
        // The dialog was closed while the password was checked: stop the session the backend just started.
        void call('transfer_send_cancel').catch(() => {})
        return
      }
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
