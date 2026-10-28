/** The shared events of the browser mock, and the «Шашлыки» board they start with. */

import { txId } from '@/shared/lib/transactions'
import { mockContacts, mockTransactionFrom } from '../ledger'
import { mockPush, mockStores, priceOf } from '../demo'
import { ME } from './engine'

export const mockSharedEvents: SharedEvent[] = []

export function fail(code: string, message: string): Promise<never> {
  return Promise.reject({ code, message })
}

export function findEvent(id: string): SharedEvent | undefined {
  return mockSharedEvents.find((event) => event.id === id)
}

const RUB: AssetIdent = { symbol: 'RUB', name: 'Рубль', coingecko_id: null, chain: 'fiat' as Chain, contract: null }

/** My payment for a purchase, in the wallet it came from. */
export function mockEventPayment(
  event: SharedEvent,
  item: EventItem,
  purchase: Purchase,
  storeId: string,
  txId?: string,
): Transaction {
  const tx = mockTransactionFrom(
    {
      type: 'payment',
      common: {
        timestamp: purchase.at,
        comment: `${event.title} · ${item.title}`,
        categories: [item.category_id ?? event.category_id ?? 'other'],
        flow_id: null,
        contact_id: null,
        price_usd: null,
        fx_override: null,
        price: null,
        price_currency: null,
      },
      store_id: storeId,
      asset: { ...RUB, symbol: purchase.currency.toUpperCase(), name: purchase.currency.toUpperCase() },
      amount: String(
        purchase.price_mode === 'per_unit' ? Number(purchase.qty) * Number(purchase.amount) : purchase.amount,
      ),
      fee: null,
      to: null,
      details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null },
    },
    priceOf,
    txId ? { id: txId, origin: 'manual' as TxOrigin } : undefined,
  )
  return mockPush(tx)
}

function purchase(id: string, buyer: string, qty: string, amount: string, tx: string | null): Purchase {
  return {
    id,
    buyer,
    qty,
    amount,
    currency: 'rub',
    price_mode: 'total',
    rate_to_event: '1',
    rate_manual: false,
    tx_id: tx,
    tx_owned: tx !== null,
    at: '2026-11-02T12:00:00Z',
  }
}

function item(id: string, title: string, unit: string, qty: string, total: string): EventItem {
  return {
    id,
    title,
    unit,
    category_id: null,
    plan: { qty, price: total, price_mode: 'total' },
    purchases: [],
    cancelled: false,
    split: { equal: { among: [] } },
  }
}

function seedContact(id: string, name: string, phone: string): Contact {
  return {
    id,
    name,
    kind: 'person',
    accounts: [
      {
        id: `${id}-phone`,
        group: null,
        label: 'Телефон',
        kind: 'phone',
        value: phone,
        currency: 'rub',
        network: null,
        active: true,
      },
    ],
    note: '',
    archived: false,
    created_at: new Date().toISOString(),
    group: '',
  }
}

/**
 * «Шашлыки · 2–4 ноя» (the design board, shared-expenses 2): five people,
 * 7 100 ₽ bought, the beer split by units. Core tests the same numbers:
 * `testing::skewers_scenario`.
 */
export function seedSharedEvents() {
  const bank = mockStores.find((store) => store.name === 'Тинькофф')
  if (!bank || mockSharedEvents.length > 0) return
  mockContacts.push(
    seedContact('mock-contact-petya', 'Петя', '+7 916 555 01 01'),
    seedContact('mock-contact-vasya', 'Вася', '+7 916 555 02 02'),
    seedContact('mock-contact-anya', 'Аня', '+7 916 555 03 03'),
  )
  const meat = item('meat', 'Мясо', 'кг', '4', '4800')
  const beer = item('beer', 'Пиво', 'шт', '6', '1200')
  const coal: EventItem = { ...item('coal', 'Уголь', 'уп', '1', '0'), plan: null }
  const event: SharedEvent = {
    id: 'mock-event-skewers',
    title: 'Шашлыки',
    created_at: new Date().toISOString(),
    note: '',
    archived: false,
    starts_on: '2026-11-02',
    ends_on: '2026-11-04',
    currency: 'rub',
    rounding_step: '1',
    category_id: null,
    organizer: ME,
    i_participate: true,
    participants: [
      { id: 'petya', name: 'Петя', contact_id: 'mock-contact-petya', details: null },
      { id: 'vasya', name: 'Вася', contact_id: 'mock-contact-vasya', details: null },
      { id: 'anya', name: 'Аня', contact_id: 'mock-contact-anya', details: null },
      {
        id: 'olya',
        name: 'Оля',
        contact_id: null,
        details: { phone: '+7 912 345-67-89', messenger: '', requisites: [], note: '' },
      },
    ],
    items: [
      meat,
      beer,
      item('veg', 'Овощи', 'кг', '3', '900'),
      item('pita', 'Лаваш', 'шт', '4', '300'),
      item('sauce', 'Соус', 'шт', '2', '400'),
      item('juice', 'Сок', 'л', '6', '1800'),
      coal,
    ],
    settlements: [],
    adjustments: [],
  }
  beer.split = { by_units: { units: { petya: '3', vasya: '5' } } }
  const meatBuy = purchase('p-meat', ME, '4', '4800', null)
  const beerBuy = purchase('p-beer', ME, '8', '1800', null)
  meatBuy.tx_id = txId(mockEventPayment(event, meat, meatBuy, bank.id))
  beerBuy.tx_id = txId(mockEventPayment(event, beer, beerBuy, bank.id))
  meatBuy.tx_owned = true
  beerBuy.tx_owned = true
  meat.purchases.push(meatBuy)
  beer.purchases.push(beerBuy)
  coal.purchases.push(purchase('p-coal', 'vasya', '1', '500', null))
  mockSharedEvents.push(event)
}
