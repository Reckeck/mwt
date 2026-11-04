import { type InjectionKey, computed, inject, provide } from 'vue'
import { useI18n } from 'vue-i18n'
import { categoryKeyLabelOrDeleted } from '@/shared/lib/categories'
import { nameOrDeleted } from '@/shared/lib/deleted-record'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useAllAttachments, useCategories, useContacts, useStoreTree } from '@/shared/api/queries'

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

  const tree = useStoreTree()
  const contacts = useContacts()
  const categories = useCategories()
  const attachments = useAllAttachments()

  const nodes = computed(() => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node])))

  const storeName = (id: string) =>
    nameOrDeleted(nodes.value.get(id)?.path.join(' › '), Boolean(tree.data.value), id, t)

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

  const contactName = (id: string) =>
    nameOrDeleted(contacts.data.value?.find((contact) => contact.id === id)?.name, Boolean(contacts.data.value), id, t)

  /** «Bybit › Spot», «Ann › IBAN», or the free text. */
  const sideLabel = (side: AccountRef | null): string => {
    if (!side) return ''
    if ('store' in side) return storeName(side.store)
    if ('raw' in side) return side.raw
    const contact = contacts.data.value?.find((entry) => entry.id === side.contact.contact_id)
    const account = contact?.accounts.find((entry) => entry.id === side.contact.account_id)
    const name = nameOrDeleted(contact?.name, Boolean(contacts.data.value), side.contact.contact_id, t)
    return account ? `${name} › ${account.label || account.value}` : name
  }

  const categoryName = (key: string) => categoryKeyLabelOrDeleted(key, categories.data.value, t)

  /** Files and links of each owner, counted once per list. */
  const attachmentCounts = computed(() => {
    const out = new Map<string, number>()
    for (const item of attachments.data.value ?? []) out.set(item.owner.id, (out.get(item.owner.id) ?? 0) + 1)
    return out
  })
  const attachmentCount = (id: string) => attachmentCounts.value.get(id) ?? 0

  return { storeName, rootOf, contactName, sideLabel, categoryName, attachmentCount }
}
