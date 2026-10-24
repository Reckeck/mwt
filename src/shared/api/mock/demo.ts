/** The demo wallets and their history, seeded on first use. */

import { txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { mockTransactionFrom } from './ledger'
import { seedUtilityDemo } from './planning'
import { mockImportSettings, mockTerminal } from './catalog'
import { mockStore } from './tree'
import { priceOf } from './prices'

/** Demo history: a salary, a payment, a trade, a debt and a conversion via an exchanger. */
export function demoTransactions(): Transaction[] {
  const now = Date.now()
  const at = (hours: number) => new Date(now - hours * 3.6e6).toISOString()
  const common = (hours: number, comment: string, categories: string[] = []): TxCommonInput => ({
    timestamp: at(hours),
    comment,
    categories,
    flow_id: null,
    contact_id: null,
    price_usd: null,
    fx_override: null,
    price: null,
    price_currency: null,
  })
  const [, binance, ledger, futures, bank] = mockStores
  const btc = { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin', chain: 'bitcoin' as Chain, contract: null }
  const usdt = { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether', chain: null, contract: null }
  const rub = { symbol: 'RUB', name: 'Рубль', coingecko_id: null, chain: 'fiat' as Chain, contract: null }
  const details: KindDetails = { cash_breakdown: null, unit_price: null, network: null, tx_hash: null }
  const inputs: TransactionInput[] = [
    {
      type: 'topup',
      common: common(240, 'Зарплата', ['salary']),
      store_id: bank.id,
      asset: rub,
      amount: '150000',
      fee: null,
      from: null,
      details,
    },
    {
      type: 'payment',
      common: common(200, 'Пятёрочка', ['groceries']),
      store_id: bank.id,
      asset: rub,
      amount: '3400',
      fee: null,
      to: { raw: 'Пятёрочка' },
      details,
    },
    {
      type: 'topup',
      common: common(190, 'Депозит', ['transfer']),
      store_id: binance.id,
      asset: usdt,
      amount: '5000',
      fee: null,
      from: null,
      details,
    },
    {
      type: 'trade_spot',
      common: common(150, 'Покупка BTC'),
      store_id: binance.id,
      sold: { asset: usdt, qty: '4500' },
      bought: { asset: btc, qty: '0.05' },
      fee: { asset: usdt, amount: '4.5' },
      liquidity: 'taker',
      order_id: null,
    },
    {
      type: 'transfer',
      common: common(120, 'На холодный'),
      from: { store: binance.id },
      to: { store: ledger.id },
      asset: btc,
      amount: '0.04',
      fee: { asset: btc, amount: '0.0002' },
      network: 'bitcoin',
      tx_hash: null,
      debt: null,
    },
    {
      type: 'transfer',
      common: common(100, 'Маржа на фьючи'),
      from: { store: binance.id },
      to: { store: futures.id },
      asset: usdt,
      amount: '400',
      fee: null,
      network: null,
      tx_hash: null,
      debt: null,
    },
    {
      type: 'transfer',
      common: common(48, 'Маме до зарплаты'),
      from: { store: bank.id },
      to: { contact: { contact_id: 'mock-contact-mom', account_id: 'mock-account-card' } },
      asset: rub,
      amount: '5000',
      fee: null,
      network: null,
      tx_hash: null,
      debt: 'lend',
    },
    {
      type: 'convert',
      common: { ...common(768, 'Обмен через skycapital'), contact_id: 'mock-contact-sky' },
      from: { account: { store: bank.id }, asset: rub, amount: '40000' },
      to: { account: { store: binance.id }, asset: usdt, amount: '432' },
      fee: null,
      rate_override: null,
    },
    {
      type: 'convert',
      common: { ...common(1488, 'Обмен через skycapital'), contact_id: 'mock-contact-sky' },
      from: { account: { store: bank.id }, asset: rub, amount: '30000' },
      to: { account: { store: binance.id }, asset: usdt, amount: '326' },
      fee: null,
      rate_override: null,
    },
    {
      type: 'convert',
      common: { ...common(24, 'Обмен через skycapital'), contact_id: 'mock-contact-sky' },
      from: { account: { store: bank.id }, asset: rub, amount: '45000' },
      to: { account: { store: binance.id }, asset: usdt, amount: '480' },
      fee: null,
      rate_override: '93.75',
    },
  ]
  return inputs.map((input) => mockTransactionFrom(input, priceOf))
}

export const newStore = (
  name: string,
  kind: StoreKind,
  color: string,
  parent: string | null = null,
  note = '',
): Store => ({
  id: crypto.randomUUID(),
  name,
  kind,
  color,
  archived: false,
  note,
  parent_id: parent,
  contact_id: null,
  liability: null,
  sub_type: null,
  binding: 'any',
  purpose: kind === 'cash' || kind === 'bank' ? 'everyday' : 'investment',
  custody: kind === 'crypto' ? 'cold' : null,
  property: null,
  secured_by: null,
  exchange:
    kind === 'exchange'
      ? { provider: 'bybit', import_interval_sec: 900, terminal: mockTerminal(), import: mockImportSettings() }
      : null,
})
export function mockMarketBinding(subType: SubType): Binding {
  if (['spot', 'futures', 'earn', 'funding', 'unified'].includes(subType)) return { market: subType as MarketKind }
  return 'any'
}

export function mockDefaultPurpose(input: CreateStoreInput): Purpose {
  if (input.sub_type === 'deposit' || input.sub_type === 'savings') return 'savings'
  if (input.sub_type && ['card', 'credit_card', 'account', 'pocket'].includes(input.sub_type)) return 'everyday'
  const parent = input.parent_id ? mockStore(input.parent_id) : undefined
  if (parent) return parent.purpose
  return input.kind === 'cash' || input.kind === 'bank' ? 'everyday' : 'investment'
}

export const mockStores: Store[] = [
  newStore('Наличные', 'cash', '#16a34a'),
  newStore('Bybit', 'exchange', '#f0b90b', null, 'Фьючи'),
  newStore('Ledger', 'crypto', '#3b82f6'),
]
// A sub-account, so the wallet tree has something to show in the browser.
mockStores.push({
  ...newStore('Futures', 'exchange', '#f0b90b', mockStores[1].id),
  sub_type: 'futures',
  binding: { market: 'futures' },
  exchange: null,
  margin: { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether', chain: null, contract: null, quote: 'USDT' },
})
mockStores.push(newStore('Тинькофф', 'bank', '#facc15'))
// A home with bills, so the utilities screens have something to show.
mockStores.push({
  ...newStore('Квартира', 'property', '#a855f7'),
  property: {
    category: 'flat',
    address: '',
    area: null,
    purchased_at: null,
    purchase_price: null,
    currency: 'rub',
    utilities_pay_day: 10,
  },
})
seedUtilityDemo(mockStores[mockStores.length - 1].id)
// A car with papers, so the car card has something to show.
mockStores.push({
  ...newStore('Машина', 'property', '#0ea5e9'),
  property: {
    category: 'car',
    address: '',
    area: null,
    purchased_at: null,
    purchase_price: null,
    currency: 'rub',
    car: {
      model: 'Kia Rio',
      year: 2019,
      plate: 'А123ВС77',
      osago_until: '2027-03-15',
      kasko_until: null,
      service_due_at: '2027-01-20',
      service_due_km: 75000,
      service_interval_months: 12,
      service_interval_km: 15000,
      mileage_km: 72400,
      mileage_at: '2026-10-01',
    },
  },
})

export const mockTransactions: Transaction[] = []

export { mockPrices, priceOf } from './prices'

export const timestampOf = (tx: Transaction) => txMeta(tx).timestamp

export function mockFilterTxs(storeIds: string[] | null): Transaction[] {
  const list = storeIds
    ? mockTransactions.filter((tx) => txLegs(tx).some((leg) => storeIds.includes(leg.store_id)))
    : mockTransactions
  return [...list].sort((left, right) => timestampOf(right).localeCompare(timestampOf(left)))
}

const daysAgo = (days: number, hour: number) => {
  const date = new Date()
  date.setDate(date.getDate() - days)
  date.setHours(hour, 0, 0, 0)
  return date.toISOString()
}

/** The notification log of the browser mock: two read ones, so the calendar has something to show. */
export const mockNotices: NoticeRecord[] = [
  {
    key: 'reminder:demo-salary',
    category: 'reminder',
    title: 'Сегодня должна прийти зарплата',
    body: 'Запишите этот перевод',
    link: '/automations/reminders',
    at: daysAgo(3, 9),
    delivered_at: daysAgo(3, 9),
    read: true,
  },
  {
    key: 'alert:demo-btc',
    category: 'price_alert',
    title: 'BTC ≥ 90 000 $',
    body: '',
    link: '/automations/alerts',
    at: daysAgo(2, 14),
    delivered_at: daysAgo(2, 14),
    read: true,
  },
]
/** Reassigned by commands: a holder, since an imported binding cannot be assigned. */
export const mockAutostart: { value: boolean } = { value: false }

export function mockPush(tx: Transaction): Transaction {
  mockTransactions.push(tx)
  return tx
}

/** Ledger balance of one asset in one store, leaving out the transaction being edited. */
export function mockBalanceOf(storeId: string, symbol: string, exclude?: string): number {
  return mockTransactions
    .filter((tx) => txId(tx) !== exclude)
    .flatMap((tx) => txLegs(tx))
    .filter((leg) => leg.store_id === storeId && leg.asset.symbol === symbol)
    .reduce((sum, leg) => sum + Number(leg.amount), 0)
}
