/** `get_entity_card` for `pnpm dev`: a simplified copy of core `relations`. */

import { txContactIds, txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { mockStores, mockTransactions } from './demo'
import { mockCategories, mockContacts } from './ledger/seed'
import { mockOverview, mockStore } from './tree'
import { goals } from './planning/state'
import { workflows } from './workflows/state'

const CAP = 20

function group(kind: RelationKind, items: RelationItem[]): RelationGroup[] {
  return items.length ? [{ kind, items: items.slice(0, CAP), total: items.length }] : []
}

function item(kind: EntityKind, id: string, label: string, extra: Partial<RelationItem> = {}): RelationItem {
  return { target: { kind, id }, label, when: null, value: null, currency: null, archived: false, ...extra }
}

function storeItem(id: string): RelationItem[] {
  const store = mockStore(id)
  return store ? [item('store', store.id, store.name, { archived: store.archived })] : []
}

const storeIdsOf = (tx: Transaction) => [...new Set(txLegs(tx).map((leg) => leg.store_id))]

function txItems(txs: Transaction[]): RelationItem[] {
  return [...txs]
    .sort((left, right) => txMeta(right).timestamp.localeCompare(txMeta(left).timestamp))
    .map((tx) => item('tx', txId(tx), txMeta(tx).comment, { when: txMeta(tx).timestamp }))
}

function relations(target: EntityRef): RelationGroup[] {
  const { id } = target
  switch (target.kind) {
    case 'store':
      return [
        ...group('parent', mockStore(id)?.parent_id ? storeItem(mockStore(id)?.parent_id ?? '') : []),
        ...group(
          'children',
          mockStores.filter((store) => store.parent_id === id).flatMap((store) => storeItem(store.id)),
        ),
        ...group('transactions', txItems(mockTransactions.filter((tx) => storeIdsOf(tx).includes(id)))),
      ]
    case 'tx': {
      const tx = mockTransactions.find((entry) => txId(entry) === id)
      if (!tx) return []
      const flow = txMeta(tx).flow_id
      return [
        ...group('stores', storeIdsOf(tx).flatMap(storeItem)),
        ...group(
          'categories',
          txMeta(tx).categories.map((key) =>
            item('category', key, mockCategories.find((cat) => cat.key === key)?.name ?? ''),
          ),
        ),
        ...group(
          'contacts',
          [...new Set(txContactIds(tx))].map((contact) =>
            item('contact', contact, mockContacts.find((entry) => entry.id === contact)?.name ?? ''),
          ),
        ),
        ...group(
          'flow',
          flow ? txItems(mockTransactions.filter((other) => txId(other) !== id && txMeta(other).flow_id === flow)) : [],
        ),
      ]
    }
    case 'category':
      return group('transactions', txItems(mockTransactions.filter((tx) => txMeta(tx).categories.includes(id))))
    case 'contact':
      return group('transactions', txItems(mockTransactions.filter((tx) => txContactIds(tx).includes(id))))
    case 'goal': {
      const stores = goals.find((goal) => goal.id === id)?.scope.store_ids ?? []
      // Like the core: workflows quoting one of the goal's wallets fill it.
      const filling = workflows.filter((flow) => stores.some((store) => flow.graph.includes(`"${store}"`)))
      return [
        ...group('stores', stores.flatMap(storeItem)),
        ...group(
          'workflows',
          filling.map((flow) => item('workflow', flow.id, flow.name, { archived: !flow.active })),
        ),
      ]
    }
    default:
      return []
  }
}

export function mockEntityCard(target: EntityRef): EntityCard {
  const card: EntityCard = {
    target,
    title: '',
    value: null,
    currency: null,
    archived: false,
    missing: false,
    groups: [],
  }
  const { id } = target
  if (target.kind === 'store') {
    const store = mockStore(id)
    if (!store) return { ...card, missing: true }
    const overview = mockOverview(id, true)
    Object.assign(card, {
      title: store.name,
      archived: store.archived,
      value: overview.subtree_base,
      currency: overview.base_currency,
    })
  } else if (target.kind === 'tx') {
    const tx = mockTransactions.find((entry) => txId(entry) === id)
    if (!tx) return { ...card, missing: true }
    Object.assign(card, { title: txMeta(tx).comment, transaction: tx })
  } else if (target.kind === 'category') {
    const category = mockCategories.find((entry) => entry.key === id)
    if (!category) return { ...card, missing: true }
    Object.assign(card, { title: category.name ?? '', archived: category.archived })
  } else if (target.kind === 'goal') {
    const goal = goals.find((entry) => entry.id === id)
    if (!goal) return { ...card, missing: true }
    Object.assign(card, { title: goal.name, value: goal.target_amount, currency: goal.currency })
  } else if (target.kind === 'contact') {
    const contact = mockContacts.find((entry) => entry.id === id)
    if (!contact) return { ...card, missing: true }
    Object.assign(card, { title: contact.name, archived: contact.archived })
  }
  return { ...card, groups: relations(target) }
}
