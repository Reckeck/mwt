import { type InjectionKey, computed, inject, provide } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

export type TxContext = ReturnType<typeof buildTxContext>

const TX_CONTEXT: InjectionKey<TxContext> = Symbol('tx-context')

/**
 * Names a transaction row needs — wallets, contacts, categories — and the
 * wallet tree's roots for spotting internal moves. One set of cached queries
 * shared by every list that renders transactions.
 */
export function useTxContext(): TxContext {
  return inject(TX_CONTEXT, null) ?? buildTxContext()
}

/**
 * One context for a whole list: its rows inject it instead of each building
 * its own maps and query observers (a page of rows did that a hundred times).
 */
export function provideTxContext(): TxContext {
  const context = useTxContext()
  provide(TX_CONTEXT, context)
  return context
}

function buildTxContext() {
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

  /** Store id → its top-level wallet's id, built once per tree. */
  const roots = computed(() => {
    const out = new Map<string, string>()
    const walk = (node: StoreTreeNode, root: string) => {
      out.set(node.store.id, root)
      for (const child of node.children) walk(child, root)
    }
    for (const top of tree.data.value ?? []) walk(top, top.store.id)
    return out
  })

  /** Top-level wallet of a store, by id. */
  const rootOf = (id: string) => roots.value.get(id) ?? id

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
