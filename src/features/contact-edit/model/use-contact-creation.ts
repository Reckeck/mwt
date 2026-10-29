import { ref, watch } from 'vue'

import type { ContactCreator } from '@/shared/lib/contact-create'

/**
 * State of the one «new contact» form the shell hosts for every picker: `create(name)`
 * opens it with the name filled in and resolves with the saved contact (`null` on close).
 */
export function useContactCreation() {
  const open = ref(false)
  const name = ref('')
  let settle: ((contact: Contact | null) => void) | null = null

  const create: ContactCreator = (typed) => {
    settle?.(null)
    name.value = typed
    open.value = true
    return new Promise((resolve) => {
      settle = resolve
    })
  }
  function saved(contact: Contact) {
    settle?.(contact)
    settle = null
  }
  // Closed without saving (Cancel, Esc, `Back`): the picker keeps what it had.
  watch(open, (now) => {
    if (now) return
    settle?.(null)
    settle = null
  })

  return { open, name, create, saved }
}
