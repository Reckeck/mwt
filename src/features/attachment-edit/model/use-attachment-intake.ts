import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { MAX_ATTACHMENT_BYTES, fileToBase64, formatBytes } from '@/entities/attachment'
import { call } from '@/shared/api/ipc'
import { errorReason } from '@/shared/lib/notify'

import type { FailedFile, PendingFile } from '../attachment-edit.types'

type AddFile = (vars: { name: string; mime: string | null; data: string }) => Promise<Attachment>

/**
 * Files on their way in (Attach 3, 7): each gets a row with a bar while it is
 * sealed and saved, and a red row with the reason if it was too big or could
 * not be read. `Undo` on a pending row drops the file once it lands.
 */
export function useAttachmentIntake(addFile: AddFile) {
  const { t } = useI18n()
  const pending = ref<PendingFile[]>([])
  const failed = ref<FailedFile[]>([])
  let next = 0

  const size = (bytes: number) =>
    formatBytes(bytes, { b: t('attachments.bytes'), kb: t('attachments.kb'), mb: t('attachments.mb') })
  const fail = (name: string, why: string) =>
    failed.value.push({ key: next++, name, reason: t('attachments.notAttached', { why }) })

  async function take(file: File) {
    if (file.size > MAX_ATTACHMENT_BYTES) {
      fail(file.name, t('attachments.tooLarge', { size: size(file.size), max: size(MAX_ATTACHMENT_BYTES) }))
      return
    }
    const entry: PendingFile = { key: next++, name: file.name || t('attachments.pastedName'), cancelled: false }
    pending.value.push(entry)
    try {
      const added = await addFile({ name: entry.name, mime: file.type || null, data: await fileToBase64(file) })
      // `entry` is the plain object; the reactive copy in the list carries the flag.
      if (pending.value.find((row) => row.key === entry.key)?.cancelled) {
        await call('remove_attachment', { input: { id: added.id } })
      }
    } catch (err) {
      fail(entry.name, errorReason(err))
    } finally {
      pending.value = pending.value.filter((row) => row.key !== entry.key)
    }
  }

  function takeAll(files: Iterable<File>) {
    for (const file of files) void take(file)
  }

  function cancel(key: number) {
    pending.value = pending.value.map((row) => (row.key === key ? { ...row, cancelled: true } : row))
  }

  function dismiss(key: number) {
    failed.value = failed.value.filter((row) => row.key !== key)
  }

  return { pending, failed, takeAll, cancel, dismiss }
}
