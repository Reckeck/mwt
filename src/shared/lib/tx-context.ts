import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

/**
 * Names a transaction row needs — wallets, contacts, categories — and the
 * wallet tree's roots for spotting internal moves. One set of cached queries
 * shared by every list that renders transactions.
 */
export function useTxContext() {
  const { t } = useI18n()

  const tree = useQuery({
    queryKey: STORE_TREE_QUERY_KEY,
    queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
  })
  const contacts = useQuery({
    queryKey: ['contacts'],
    queryFn: () => call<Contact[]>('list_contacts'),
  })
  const categories = useQuery({
    queryKey: ['categories'],
    queryFn: () => call<Category[]>('list_categories'),
  })

  const nodes = computed(() => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node])))

  const storeName = (id: string) => nodes.value.get(id)?.path.join(' › ') ?? id

  /** Top-level wallet of a store, by id. */
  const rootOf = (id: string) => {
    const node = nodes.value.get(id)
    if (!node || node.depth === 0) return id
    const rootName = node.path[0]
    for (const candidate of nodes.value.values()) {
      if (candidate.depth === 0 && candidate.store.name === rootName) return candidate.store.id
    }
    return id
  }

  const contactName = (id: string) => contacts.data.value?.find((contact) => contact.id === id)?.name ?? id

  /** «Bybit › Spot», «Ann › IBAN», or the free text. */
  const sideLabel = (side: AccountRef | null): string => {
    if (!side) return ''
    if ('store' in side) return storeName(side.store)
    if ('raw' in side) return side.raw
    const contact = contacts.data.value?.find((entry) => entry.id === side.contact.contact_id)
    const account = contact?.accounts.find((entry) => entry.id === side.contact.account_id)
    const name = contact?.name ?? side.contact.contact_id
    return account ? `${name} › ${account.label || account.value}` : name
  }

  const categoryName = (key: string) => categoryKeyLabel(key, categories.data.value, t)

  return { storeName, rootOf, contactName, sideLabel, categoryName }
}
