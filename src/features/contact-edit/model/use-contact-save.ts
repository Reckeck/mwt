import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { call, parseError } from '@/shared/api/ipc'
import { codeErrorMessage } from '@/shared/lib/vault-error'

/** The full draft `update_contact` takes, from the stored contact plus a patch. */
export function draftOf(contact: Contact, patch: Partial<ContactDraft> = {}): ContactDraft {
  return {
    name: contact.name,
    kind: contact.kind,
    note: contact.note,
    accounts: contact.accounts.map((account) => ({ ...account })),
    group: contact.group,
    ...patch,
  }
}

/** Autosave for inline edits: quiet on success, loud on failure. */
export function useContactSave(contactId: () => string) {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (draft: ContactDraft) =>
      call<Contact>('update_contact', { input: { id: contactId(), contact: draft } }),
    onSuccess: (contact) => {
      queryClient.setQueryData<Contact[]>(['contacts'], (old) =>
        old?.map((entry) => (entry.id === contact.id ? contact : entry)),
      )
      void queryClient.invalidateQueries({ queryKey: ['contacts'] })
    },
    onError: (err) => toast.error(t('contacts.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
  })
}
