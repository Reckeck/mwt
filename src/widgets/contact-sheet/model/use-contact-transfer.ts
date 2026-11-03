import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'

/** A transfer or a debt operation started from a contact, optionally to one of its requisites. */
export interface ContactTransfer {
  effect: DebtEffect | null
  accountId: string | null
}

/**
 * The «Record a transfer» form of the open contact. Operations with a contact start from the first
 * everyday wallet (changeable in the form); an archived contact or no wallet blocks it, with the reason.
 */
export function useContactTransfer(contact: () => Contact | null) {
  const { t } = useI18n()
  const tree = useStoreTree()

  const open = ref(false)
  const transfer = ref<ContactTransfer>({ effect: null, accountId: null })

  const storeId = computed(() => {
    const nodes = flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived)
    return (nodes.find((node) => node.store.purpose === 'everyday') ?? nodes[0])?.store.id ?? ''
  })
  const blocked = computed<string | false>(() => {
    if (contact()?.archived) return t('contacts.transferArchived')
    return storeId.value ? false : t('contacts.transferNoStore')
  })
  const counterpart = computed<AccountRef | null>(() => {
    const current = contact()
    return current ? { contact: { contact_id: current.id, account_id: transfer.value.accountId } } : null
  })

  function start(next: ContactTransfer) {
    transfer.value = next
    open.value = true
  }

  return { open, transfer, storeId, blocked, counterpart, start }
}
