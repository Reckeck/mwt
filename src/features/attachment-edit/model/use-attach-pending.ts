import { useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'

import { type PendingAttachment, pendingLabel } from '../lib/pending-attachment'

/**
 * Attaches what was picked in a form to the operation that was just saved. A failed one does not
 * undo the operation: the owner is told which were not attached.
 */
export function useAttachPending() {
  const queryClient = useQueryClient()
  const { t } = useI18n()

  return async function attach(txId: string, items: PendingAttachment[]) {
    if (!items.length) return
    const owner: EntityRef = { kind: 'tx', id: txId }
    const missed: string[] = []
    for (const item of items) {
      try {
        if (item.kind === 'link')
          await call<Attachment>('add_attachment_link', { input: { owner, url: item.url, title: '' } })
        else
          await call<Attachment>('add_attachment_file', {
            input: { owner, name: item.name, mime: item.mime, data_base64: item.dataBase64 },
          })
      } catch {
        missed.push(pendingLabel(item))
      }
    }
    void queryClient.invalidateQueries({ queryKey: ['attachments'] })
    if (missed.length) notify.error(null, t('attachments.stagedMissed', { names: missed.join(', ') }))
  }
}
