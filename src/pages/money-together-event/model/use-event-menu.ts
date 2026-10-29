import { type MaybeRefOrGetter, ref, toValue } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { Archive, ArchiveRestore, Pencil, Trash2 } from 'lucide-vue-next'

import { useArchiveSharedEvent, useDeleteSharedEvent } from '@/entities/shared-event'
import type { UiAction } from '@/shared/ui/context-menu'

/** «⋯» of the event: change, archive (a closed event only), delete; plus the dialogs those open. */
export function useEventMenu(view: MaybeRefOrGetter<SharedEventView | undefined>) {
  const { t } = useI18n()
  const router = useRouter()
  const editOpen = ref(false)
  const dropOpen = ref(false)
  const archive = useArchiveSharedEvent()
  const remove = useDeleteSharedEvent()

  function actions(): UiAction[] {
    const current = toValue(view)
    if (!current) return []
    const { event, status } = current
    const canArchive = event.archived || status === 'closed' || status === 'planning'
    return [
      { id: 'edit', label: t('sharedEvents.menu.edit'), icon: Pencil, run: () => (editOpen.value = true) },
      {
        id: 'archive',
        label: event.archived ? t('sharedEvents.menu.unarchive') : t('sharedEvents.menu.archive'),
        icon: event.archived ? ArchiveRestore : Archive,
        disabled: canArchive ? false : t('sharedEvents.menu.archiveBlocked'),
        run: () => archive.mutate({ id: event.id, archived: !event.archived, title: event.title }),
      },
      {
        id: 'delete',
        label: t('sharedEvents.menu.delete'),
        icon: Trash2,
        destructive: true,
        run: () => (dropOpen.value = true),
      },
    ]
  }

  function confirmDelete() {
    const current = toValue(view)
    if (!current) return
    remove.mutate(
      { id: current.event.id, title: current.event.title },
      {
        onSuccess: () => {
          dropOpen.value = false
          void router.replace({ name: 'money-together' })
        },
      },
    )
  }

  return { editOpen, dropOpen, actions, confirmDelete, deleting: remove.isPending }
}
