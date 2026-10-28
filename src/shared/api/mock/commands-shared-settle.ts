/** Commands: shared events, items, purchases and settling. */

import { txId } from '@/shared/lib/transactions'
import { mockPrices } from './demo'
import { mockStores, mockTransactions } from './demo'
import type { MockCommands } from './core'
import { eventOrFail, viewOf } from './commands-shared-events'
import { ME } from './shared-events/engine'
import { fail, mockEventPayment } from './shared-events/state'

const bad = (message: string) => fail(message.startsWith('participant') ? 'MISSING' : 'INVALID_INPUT', message)

const moment = (raw: string | null | undefined) => {
  const text = raw?.trim()
  if (!text) return new Date().toISOString()
  return /^\d{4}-\d{2}-\d{2}$/.test(text) ? `${text}T12:00:00Z` : text
}

function dropTx(id: string | null) {
  const index = mockTransactions.findIndex((tx) => txId(tx) === id)
  if (index >= 0) mockTransactions.splice(index, 1)
}

/** Event-currency units per unit of `currency`, from the mock's USD prices. */
function mockRate(event: SharedEvent, currency: string): number | null {
  const from = mockPrices[currency.toUpperCase()]
  const to = mockPrices[event.currency.toUpperCase()]
  return from && to ? from / to : null
}

function findPurchase(event: SharedEvent, purchaseId: string) {
  for (const item of event.items) {
    const purchase = item.purchases.find((entry) => entry.id === purchaseId)
    if (purchase) return { item, purchase }
  }
  return null
}

/** A draft turned into a purchase (without its payment), or the reason it is refused. */
function purchaseFrom(event: SharedEvent, draft: SharedPurchaseDraft, id: string): Purchase | string {
  const buyer = draft.buyer.trim() || ME
  if (buyer === ME ? !event.i_participate : !event.participants.some((person) => person.id === buyer)) {
    return buyer === ME ? 'you do not take part in this event' : `participant ${buyer}`
  }
  if (!(Number(draft.qty) > 0)) return 'the quantity must be positive'
  if (!(Number(draft.amount) > 0)) return 'the amount must be positive'
  const currency = draft.currency.trim().toLowerCase() || event.currency
  let rate = 1
  if (currency !== event.currency)
    rate = Number(draft.rate_to_event) > 0 ? Number(draft.rate_to_event) : (mockRate(event, currency) ?? 0)
  if (!rate) return `FX_MISSING ${currency.toUpperCase()}`
  if (buyer === ME && !draft.store_id) return 'choose the wallet the payment comes from'
  return {
    id,
    buyer,
    qty: draft.qty,
    amount: draft.amount,
    currency,
    price_mode: draft.price_mode,
    rate_to_event: String(rate),
    rate_manual: Number(draft.rate_to_event) > 0 && currency !== event.currency,
    tx_id: null,
    tx_owned: false,
    at: moment(draft.at),
  }
}

function itemFor(
  event: SharedEvent,
  itemId: string | null,
  title: string | null,
  unit: string | null,
): EventItem | string {
  if (itemId) return event.items.find((item) => item.id === itemId && !item.cancelled) ?? `item ${itemId}`
  if (!title?.trim()) return 'choose an item or name the new one'
  const item: EventItem = {
    id: crypto.randomUUID(),
    title: title.trim(),
    unit: unit?.trim() || 'шт',
    category_id: null,
    plan: null,
    purchases: [],
    cancelled: false,
    split: { equal: { among: [] } },
  }
  event.items.push(item)
  return item
}

function itemFrom(
  draft: SharedItemDraft,
): Pick<EventItem, 'title' | 'unit' | 'category_id' | 'plan' | 'cancelled'> | string {
  if (!draft.title.trim()) return 'an item needs a title'
  return {
    title: draft.title.trim(),
    unit: draft.unit.trim() || 'шт',
    category_id: draft.category_id,
    plan: draft.plan,
    cancelled: draft.cancelled,
  }
}

function fail2(message: string) {
  return message.startsWith('FX_MISSING') ? fail('FX_MISSING', message) : bad(message)
}

export const sharedSettleCommands: MockCommands = {
  add_shared_item: ({ body }) => {
    const input = body as SharedItemInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const fields = itemFrom(input.item)
    if (typeof fields === 'string') return bad(fields)
    event.items.push({ id: crypto.randomUUID(), purchases: [], split: { equal: { among: [] } }, ...fields })
    return viewOf(event)
  },
  update_shared_item: ({ body }) => {
    const input = body as UpdateSharedItemInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const item = event.items.find((entry) => entry.id === input.item_id)
    const fields = itemFrom(input.item)
    if (!item) return bad(`item ${input.item_id}`)
    if (typeof fields === 'string') return bad(fields)
    if (fields.cancelled && item.purchases.length) return bad('an item with purchases cannot be cancelled')
    Object.assign(item, fields)
    return viewOf(event)
  },
  delete_shared_item: ({ body }) => {
    const input = body as SharedItemRef
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const item = event.items.find((entry) => entry.id === input.item_id)
    if (!item) return bad(`item ${input.item_id}`)
    if (item.purchases.length) return bad('delete the purchases of this item first')
    event.items = event.items.filter((entry) => entry !== item)
    return viewOf(event)
  },
  set_shared_item_split: ({ body }) => {
    const input = body as SetSharedItemSplitInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const item = event.items.find((entry) => entry.id === input.item_id)
    if (!item) return bad(`item ${input.item_id}`)
    const known = (id: string) =>
      (id === ME && event.i_participate) || event.participants.some((person) => person.id === id)
    const named = 'equal' in input.split ? input.split.equal.among : Object.keys(input.split.by_units.units)
    const unknown = named.find((id) => !known(id))
    if (unknown) return bad(`participant ${unknown}`)
    item.split = input.split
    return viewOf(event)
  },
  add_shared_purchase: ({ body }) => {
    const input = body as AddSharedPurchaseInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const draft = input.purchase
    const purchase = purchaseFrom(event, draft, crypto.randomUUID())
    if (typeof purchase === 'string') return fail2(purchase)
    const store = draft.store_id ? mockStores.find((entry) => entry.id === draft.store_id) : null
    if (purchase.buyer === ME && !store) return bad('no such wallet')
    const created = event.items.length
    const item = itemFor(event, draft.item_id, draft.new_item_title, draft.new_item_unit)
    if (typeof item === 'string') return bad(item)
    if (purchase.buyer === ME && store) {
      purchase.tx_id = txId(mockEventPayment(event, item, purchase, store.id))
      purchase.tx_owned = true
    }
    item.purchases.push(purchase)
    void created
    return viewOf(event)
  },
  update_shared_purchase: ({ body }) => {
    const input = body as UpdateSharedPurchaseInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const found = findPurchase(event, input.purchase_id)
    if (!found) return bad(`purchase ${input.purchase_id}`)
    const { item, purchase: old } = found
    if (old.tx_id && !old.tx_owned) {
      old.qty = input.purchase.qty
      return viewOf(event)
    }
    const next = purchaseFrom(event, input.purchase, old.id)
    if (typeof next === 'string') return fail2(next)
    dropTx(old.tx_id)
    if (next.buyer === ME && input.purchase.store_id) {
      next.tx_id = txId(mockEventPayment(event, item, next, input.purchase.store_id, old.tx_id ?? undefined))
      next.tx_owned = true
    }
    Object.assign(old, next)
    return viewOf(event)
  },
  delete_shared_purchase: ({ body }) => {
    const input = body as SharedPurchaseRef
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const found = findPurchase(event, input.purchase_id)
    if (!found) return bad(`purchase ${input.purchase_id}`)
    if (found.purchase.tx_owned) dropTx(found.purchase.tx_id)
    found.item.purchases = found.item.purchases.filter((entry) => entry !== found.purchase)
    return viewOf(event)
  },
  attach_payment_to_shared_event: ({ body }) => {
    const input = body as AttachPaymentInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const tx = mockTransactions.find((entry) => txId(entry) === input.tx_id)
    if (!tx || !('payment' in tx)) return bad('only a payment can be a purchase of an event')
    if (event.items.some((entry) => entry.purchases.some((purchase) => purchase.tx_id === input.tx_id))) {
      return bad('this payment belongs to an event already')
    }
    const item = itemFor(event, input.item_id, input.new_item_title, null)
    if (typeof item === 'string') return bad(item)
    const payment = tx.payment
    item.purchases.push({
      id: crypto.randomUUID(),
      buyer: ME,
      qty: input.qty ?? '1',
      amount: payment.amount,
      currency: payment.asset.symbol.toLowerCase(),
      price_mode: 'total',
      rate_to_event: String(mockRate(event, payment.asset.symbol) ?? 1),
      rate_manual: false,
      tx_id: input.tx_id,
      tx_owned: false,
      at: payment.meta.timestamp,
    })
    return viewOf(event)
  },
  set_shared_share_override: ({ body }) => {
    const input = body as SetShareOverrideInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    event.adjustments = event.adjustments.filter((fix) => fix.participant_id !== input.participant_id)
    if (input.total !== null) event.adjustments.push({ participant_id: input.participant_id, total: input.total })
    return viewOf(event)
  },
}
