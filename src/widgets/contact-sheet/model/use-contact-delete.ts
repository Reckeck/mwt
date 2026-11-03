import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

/** Deletes a contact; the backend refuses one that operations refer to, and the toast says to archive it. */
export function useContactDelete(onDeleted: () => void) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (contact: Contact) => call('delete_contact', { id: contact.id }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['contacts'] })
      onDeleted()
    },
    meta: { error: 'contacts.deleteFailed', success: 'contacts.deleted' },
  })
}
