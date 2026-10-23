import { computed, type MaybeRefOrGetter, toValue } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'

import type { LinkDraft } from '@/entities/attachment'

interface NewFileVars {
  name: string
  mime: string | null
  data: string
}

/**
 * One owner's files and links and everything done to them. Each change also
 * refreshes the operations: their paperclips, the «С файлами» filter and the
 * text search read the attachments too.
 */
export function useAttachments(owner: MaybeRefOrGetter<EntityRef>, enabled: MaybeRefOrGetter<boolean> = true) {
  const { t } = useI18n()
  const queryClient = useQueryClient()

  const list = useQuery({
    // no placeholder: another owner's files
    queryKey: computed(() => ['attachments', toValue(owner).kind, toValue(owner).id]),
    queryFn: () => call<Attachment[]>('list_attachments', { input: { owner: toValue(owner) } }),
    enabled: computed(() => toValue(enabled)),
  })

  const refresh = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: ['attachments'] }),
      queryClient.invalidateQueries({ queryKey: ['transactions'] }),
    ])

  const addFile = useMutation({
    mutationFn: ({ name, mime, data }: NewFileVars) =>
      call<Attachment>('add_attachment_file', { input: { owner: toValue(owner), name, mime, data_base64: data } }),
    onSuccess: refresh,
    meta: { silent: 'the new row is the outcome; a failure becomes a red row in the list' },
  })

  const addLink = useMutation({
    mutationFn: ({ url, title }: LinkDraft) =>
      call<Attachment>('add_attachment_link', { input: { owner: toValue(owner), url, title } }),
    onSuccess: refresh,
    meta: {
      success: 'attachments.linkAdded',
      error: 'attachments.linkFailed',
      undo: (data) => call('remove_attachment', { input: { id: (data as Attachment).id } }),
    },
  })

  const remove = useMutation({
    mutationFn: (item: Attachment) => call<Attachment>('remove_attachment', { input: { id: item.id } }),
    onSuccess: refresh,
    meta: {
      success: (data) => t('attachments.removed', { title: (data as Attachment).title }),
      error: 'attachments.removeFailed',
      undo: (data) => call('restore_attachment', { input: { id: (data as Attachment).id } }),
    },
  })

  const rename = useMutation({
    mutationFn: ({ id, title }: { id: string; title: string }) =>
      call<Attachment>('rename_attachment', { input: { id, title } }),
    onSuccess: refresh,
    meta: { success: 'attachments.renamed', error: 'attachments.renameFailed' },
  })

  const saveAs = useMutation({
    mutationFn: (item: Attachment) => call<string | null>('save_attachment_as', { input: { id: item.id } }),
    meta: {
      // An empty text: the dialog was closed, nothing to say.
      success: (data) => (data ? t('attachments.saved', { path: data as string }) : ''),
      error: 'attachments.saveFailed',
    },
  })

  /** Opening changes nothing, so it is a call, not a mutation; a failure still says why. */
  async function open(item: Attachment) {
    try {
      await call('open_attachment', { input: { id: item.id } })
    } catch (err) {
      notify.error(err, 'attachments.openFailed')
    }
  }

  return { list, addFile, addLink, remove, rename, saveAs, open }
}
