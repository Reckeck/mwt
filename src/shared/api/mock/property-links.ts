/** Operations linked to a property (`VaultDoc.tx_properties`), as `service/property_links.rs` (simplified). */

import { txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { mockTransactions, priceOf } from './demo'

export const mockPropertyLinks: TxProperty[] = []

/** Link the operation to the property, or unlink it with `null`. */
export function mockLinkProperty(id: string, propertyId: string | null | undefined) {
  const kept = mockPropertyLinks.findIndex((row) => row.tx_id === id)
  if (kept >= 0) mockPropertyLinks.splice(kept, 1)
  if (propertyId) mockPropertyLinks.push({ tx_id: id, property_id: propertyId })
}

/** Rent (money in) and costs (money out) of the property's linked operations over a year, in USD. */
export function mockPropertyYear(propertyId: string): { rent: number; expenses: number } {
  const since = Date.now() - 365 * 86_400_000
  const ids = new Set(mockPropertyLinks.filter((row) => row.property_id === propertyId).map((row) => row.tx_id))
  let rent = 0
  let expenses = 0
  for (const tx of mockTransactions) {
    if (!ids.has(txId(tx)) || Date.parse(txMeta(tx).timestamp) < since) continue
    for (const leg of txLegs(tx)) {
      if (leg.store_id === propertyId) continue
      const value = Number(leg.amount) * priceOf(leg.asset.symbol)
      if ('topup' in tx && value > 0) rent += value
      else if (value < 0) expenses -= value
    }
  }
  return { rent, expenses }
}
