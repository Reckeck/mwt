import { type Ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useRouter } from 'vue-router'
import {
  Archive,
  ArchiveRestore,
  ArrowDownLeft,
  ArrowUpRight,
  Pencil,
  ScrollText,
  Trash2,
  Undo2,
} from 'lucide-vue-next'

import type { UiAction } from '@/shared/ui/context-menu'
import { call } from '@/shared/api/ipc'

import type { ContactTransfer } from './use-contact-transfer'

/** What the «⋯» of the contact's card holds: debt operations, the journal, edit, archive, delete. */
export function useContactMenu(options: {
  contact: () => Contact
  blocked: Ref<string | false>
  transfer: (next: ContactTransfer) => void
  edit: () => void
  askDelete: () => void
}) {
  const { t } = useI18n()
  const router = useRouter()
  const queryClient = useQueryClient()

  const archive = useMutation({
    mutationFn: (contact: Contact) => call('archive_contact', { id: contact.id, archived: !contact.archived }),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['contacts'] }),
    meta: {
      error: 'contacts.saveFailed',
      success: (_data, contact) => ((contact as Contact).archived ? t('contacts.unarchived') : t('contacts.archived')),
      undo: (_data, contact) =>
        call('archive_contact', { id: (contact as Contact).id, archived: (contact as Contact).archived }),
    },
  })

  return computed<UiAction[]>(() => {
    const contact = options.contact()
    const debt = (effect: DebtEffect, icon: UiAction['icon']): UiAction => ({
      id: effect,
      label: t(`debt.action.${effect}`),
      icon,
      disabled: options.blocked.value,
      run: () => options.transfer({ effect, accountId: null }),
    })
    return [
      debt('lend', ArrowUpRight),
      debt('borrow', ArrowDownLeft),
      debt('repay', Undo2),
      debt('collect', Undo2),
      {
        id: 'transactions',
        label: t('contacts.allTransactions'),
        icon: ScrollText,
        separated: true,
        run: () => void router.push({ name: 'transactions', query: { contact: contact.id } }),
      },
      { id: 'edit', label: t('contacts.edit'), icon: Pencil, run: options.edit },
      {
        id: 'archive',
        label: contact.archived ? t('stores.unarchive') : t('stores.archive'),
        icon: contact.archived ? ArchiveRestore : Archive,
        separated: true,
        run: () => archive.mutate(contact),
      },
      { id: 'delete', label: t('contacts.delete'), icon: Trash2, destructive: true, run: options.askDelete },
    ]
  })
}
