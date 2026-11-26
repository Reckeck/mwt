import { useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'

/**
 * Links a saved expense to a shared event as a purchase. The expense is saved either way; a
 * refusal of the event (a crypto asset, a closed event) is named in a toast.
 */
export function useLinkToEvent() {
  const queryClient = useQueryClient()
  const { t } = useI18n()

  return async function link(txId: string, eventId: string, title: string) {
    if (!eventId) return
    try {
      await call<SharedEventView>('attach_payment_to_shared_event', {
        input: {
          event_id: eventId,
          tx_id: txId,
          item_id: null,
          new_item_title: title.trim() || t('tx.field.eventItem'),
          qty: null,
        } satisfies AttachPaymentInput,
      })
      void queryClient.invalidateQueries({ queryKey: ['shared-events'] })
      notify.success('tx.field.eventLinked')
    } catch (err) {
      notify.error(err, 'tx.field.eventLinkFailed')
    }
  }
}
