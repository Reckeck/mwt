/** Commands: several operations at once (`service/bulk.rs`), all or nothing. */

import { txId, txMeta } from '@/shared/lib/transactions'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockTransactions } from './demo'
import { mockLinkProperty, mockPropertyLinks } from './property-links'

function indexOf(id: string): number {
  const index = mockTransactions.findIndex((tx) => txId(tx) === id)
  if (index < 0) throw { code: 'MISSING', message: 'transaction' }
  return index
}

/** The wallet field of a one-sided operation, or nothing for transfers and exchanges. */
function storeOf(tx: Transaction): { store_id: string } | null {
  if ('payment' in tx) return tx.payment
  if ('topup' in tx) return tx.topup
  return null
}

export const bulkCommands: MockCommands = {
  delete_transactions: ({ args }) => {
    const ids = (args?.ids as string[]) ?? []
    ids.forEach(indexOf)
    const deleted: DeletedTransaction[] = ids.map((id) => {
      const property_id = mockPropertyLinks.find((row) => row.tx_id === id)?.property_id ?? null
      const [transaction] = mockTransactions.splice(indexOf(id), 1)
      mockLinkProperty(id, null)
      return { transaction, property_id }
    })
    return resolve(deleted)
  },
  restore_transactions: ({ args }) => {
    for (const item of (args?.items as DeletedTransaction[]) ?? []) {
      mockTransactions.push(item.transaction)
      mockLinkProperty(txId(item.transaction), item.property_id)
    }
    return resolve(null)
  },
  set_transaction_categories: ({ args }) => {
    const changes = (args?.changes as TxCategories[]) ?? []
    for (const change of changes) indexOf(change.id)
    const before = changes.map((change) => {
      const meta = txMeta(mockTransactions[indexOf(change.id)])
      const was = { id: change.id, categories: [...meta.categories] }
      meta.categories = [...change.categories]
      return was
    })
    return resolve(before)
  },
  move_transactions: ({ args }) => {
    const changes = (args?.changes as TxStore[]) ?? []
    const sides = changes.map((change) => storeOf(mockTransactions[indexOf(change.id)]))
    if (sides.some((side) => !side))
      throw { code: 'INVALID_INPUT', message: 'only a payment or a top-up can move to another wallet' }
    const before = changes.map((change, index) => {
      const side = sides[index] as { store_id: string }
      const was = { id: change.id, store_id: side.store_id }
      side.store_id = change.store_id
      return was
    })
    return resolve(before)
  },
}
