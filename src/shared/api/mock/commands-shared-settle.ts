/** Commands: shared events, items, purchases and settling. */

import { txId } from '@/shared/lib/transactions'
import { SCALE, dec, divRound, fmt, roundTo } from './shared-events/dec'
import { mockPrices } from './demo'
import { mockStores, mockTransactions } from './demo'
import type { MockCommands } from './core'
import { eventOrFail, viewOf } from './commands-shared-events'
import { ME, computeSplit, overridesFit, sharerPool } from './shared-events/engine'
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

/** Event-currency units per unit of `currency`, from the mock's USD prices, to six places like the cache rate. */
function mockRate(event: SharedEvent, currency: string): bigint | null {
  const from = dec(String(mockPrices[currency.toUpperCase()] ?? 0))
  const to = dec(String(mockPrices[event.currency.toUpperCase()] ?? 0))
  return from > 0n && to > 0n ? roundTo(divRound(from * SCALE, to), 100n) : null
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
  if (!(dec(draft.qty) > 0n)) return 'the quantity must be positive'
  if (!(dec(draft.amount) > 0n)) return 'the amount must be positive'
  const currency = draft.currency.trim().toLowerCase() || event.currency
  const foreign = currency !== event.currency.toLowerCase()
  const typed = draft.rate_to_event !== null && draft.rate_to_event !== undefined
  let rate = SCALE
  if (foreign && typed) {
    if (!(dec(draft.rate_to_event) > 0n)) return 'the rate must be positive'
    rate = dec(draft.rate_to_event)
  } else if (foreign) {
    const found = mockRate(event, currency)
    if (found === null) return `FX_MISSING ${currency.toUpperCase()}`
    rate = found
  }
  if (buyer === ME && !draft.store_id) return 'choose the wallet the payment comes from'
  return {
    id,
    buyer,
    qty: draft.qty,
    amount: draft.amount,
    currency,
    price_mode: draft.price_mode,
    rate_to_event: fmt(rate),
    rate_manual: foreign && typed,
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
    if (!event.i_participate) return bad('you do not take part in this event')
    const tx = mockTransactions.find((entry) => txId(entry) === input.tx_id)
    if (!tx || !('payment' in tx)) return bad('only a payment can be a purchase of an event')
    if (tx.payment.asset.chain !== 'fiat') return bad('a purchase is paid in a currency, not in an asset')
    const currency = tx.payment.asset.symbol.toLowerCase()
    const rate = currency === event.currency.toLowerCase() ? SCALE : mockRate(event, currency)
    if (rate === null) return fail('FX_MISSING', `no exchange rate for ${currency.toUpperCase()}`)
    if (event.items.some((entry) => entry.purchases.some((purchase) => purchase.tx_id === input.tx_id))) {
      return bad('this payment belongs to an event already')
    }
    const item = itemFor(event, input.item_id, input.new_item_title, null)
    if (typeof item === 'string') return bad(item)
    const payment = tx.payment
    item.purchases.push({
      id: crypto.randomUUID(),
      buyer: ME,
      qty: input.qty && dec(input.qty) > 0n ? input.qty : '1',
      amount: payment.amount,
      currency,
      price_mode: 'total',
      rate_to_event: fmt(rate),
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
    const pool = sharerPool(event)
    if (!pool.includes(input.participant_id)) return bad(`participant ${input.participant_id}`)
    const adjustments = event.adjustments.filter((fix) => fix.participant_id !== input.participant_id)
    if (input.total !== null) {
      if (dec(input.total) < 0n) return bad('a total cannot be negative')
      adjustments.push({ participant_id: input.participant_id, total: input.total })
    }
    const spent = computeSplit({ ...event, adjustments: [] }).spent
    const fixed = adjustments.map((fix): [string, bigint] => [fix.participant_id, dec(fix.total)])
    if (!overridesFit(fixed, pool, spent)) {
      return bad('the fixed totals must not exceed what was bought, and must add up to it when everybody is fixed')
    }
    event.adjustments = adjustments
    return viewOf(event)
  },
}
