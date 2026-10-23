import type { Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Archive, ArchiveRestore, ArrowDownLeft, ArrowRightLeft, ArrowUpRight, Copy } from 'lucide-vue-next'

import { maskDetail } from '@/entities/contact'
import type { UiAction } from '@/shared/ui/context-menu'
import { call } from '@/shared/api/ipc'
import { copyText } from '@/shared/lib/clipboard'

/** Right-click a contact in the list: the common operations without selecting it first. */
export function useContactActions(
  transferStore: Ref<string>,
  openTransfer: (contact: Contact, effect: DebtEffect | null) => void,
) {
  const { t } = useI18n()
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

  return function contactActions(contact: Contact): UiAction[] {
    const run = (effect: DebtEffect | null) => () => openTransfer(contact, effect)
    const blocked = contact.archived
      ? t('contacts.archivedHint')
      : !transferStore.value
        ? t('contacts.noWallet')
        : false
    return [
      { id: 'transfer', label: t('contacts.writeTransfer'), icon: ArrowRightLeft, disabled: blocked, run: run(null) },
      { id: 'lend', label: t('debt.action.lend'), icon: ArrowUpRight, disabled: blocked, run: run('lend') },
      { id: 'borrow', label: t('debt.action.borrow'), icon: ArrowDownLeft, disabled: blocked, run: run('borrow') },
      {
        id: 'copy',
        label: t('contacts.copyAccount'),
        icon: Copy,
        separated: true,
        hidden: !contact.accounts.length,
        children: contact.accounts.map((account, index) => ({
          id: `copy-${account.id || index}`,
          label: `${account.label || t(`detailKind.${account.kind}`)} · ${maskDetail(account.kind, account.value)}`,
          run: () => void copyText(account.value),
        })),
      },
      {
        id: 'archive',
        label: contact.archived ? t('stores.unarchive') : t('stores.archive'),
        icon: contact.archived ? ArchiveRestore : Archive,
        destructive: !contact.archived,
        separated: true,
        run: () => archive.mutate(contact),
      },
    ]
  }
}
