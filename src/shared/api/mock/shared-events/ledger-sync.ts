/**
 * When the ledger changes an operation an event made or adopted, the event
 * follows (`service/shared_events/ledger.rs`): deleting removes the purchase
 * or the settlement, editing the amount or date edits them, and what the event
 * cannot follow is refused.
 */

import { dec, fmt } from './dec'
import { SETTLE_TOLERANCE, computeSplit } from './engine'
import { mockSharedEvents } from './state'

const CHANGE_IN_EVENT = 'this operation belongs to a shared event: change it in the event'

/** Drop the purchases and settlements that point at an operation about to go. */
export function cascadeSharedDelete(id: string) {
  for (const event of mockSharedEvents) {
    let touched = false
    for (const item of event.items) {
      const kept = item.purchases.filter((purchase) => purchase.tx_id !== id)
      touched ||= kept.length !== item.purchases.length
      item.purchases = kept
    }
    const kept = event.settlements.filter((settlement) => settlement.tx_id !== id)
    touched ||= kept.length !== event.settlements.length
    event.settlements = kept
    if (touched) event.archived = false
  }
}

/** The event as it will be once `id` becomes `next`: `null` when no event owns it, a string when refused. */
export function planSharedTxEdit(id: string, old: Transaction, next: Transaction): SharedEvent | string | null {
  for (const event of mockSharedEvents) {
    const copy = structuredClone(event)
    let found = false
    for (const item of copy.items) {
      const purchase = item.purchases.find((entry) => entry.tx_id === id)
      if (!purchase) continue
      if (!('payment' in next)) return CHANGE_IN_EVENT
      const payment = next.payment
      if (payment.asset.chain !== 'fiat' || payment.asset.symbol.toLowerCase() !== purchase.currency.toLowerCase()) {
        return CHANGE_IN_EVENT
      }
      purchase.amount = payment.amount
      purchase.price_mode = 'total'
      purchase.at = payment.meta.timestamp
      found = true
    }
    const index = copy.settlements.findIndex((entry) => entry.tx_id === id)
    if (index >= 0) {
      if (!('transfer' in next) || !('transfer' in old)) return CHANGE_IN_EVENT
      const now = next.transfer
      if (
        now.debt !== old.transfer.debt ||
        now.asset.symbol.toLowerCase() !== copy.currency.toLowerCase() ||
        !(dec(now.amount) > 0n)
      ) {
        return CHANGE_IN_EVENT
      }
      const [settlement] = copy.settlements.splice(index, 1)
      const saldo = computeSplit(copy).people.find((row) => row.id === settlement.participant_id)?.saldo ?? 0n
      const allowed = settlement.direction === 'to_organizer' ? saldo : -saldo
      if (dec(now.amount) > allowed + SETTLE_TOLERANCE) return 'this is more than is owed in that direction'
      settlement.amount = fmt(dec(now.amount))
      settlement.at = now.meta.timestamp
      copy.settlements.splice(index, 0, settlement)
      found = true
    }
    if (found) {
      copy.archived = false
      return copy
    }
  }
  return null
}

export function applySharedEdit(next: SharedEvent) {
  const index = mockSharedEvents.findIndex((event) => event.id === next.id)
  if (index >= 0) mockSharedEvents[index] = next
}

export const ownsTx = (id: string) =>
  mockSharedEvents.some(
    (event) =>
      event.items.some((item) => item.purchases.some((purchase) => purchase.tx_id === id)) ||
      event.settlements.some((settlement) => settlement.tx_id === id),
  )
