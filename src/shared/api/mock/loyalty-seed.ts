import { mockTransactionFrom } from './ledger'
import { mockPrices, mockStores, mockTransactions, newStore, priceOf } from './demo'
import { STRESS_DETAILS } from './demo-year'

/** The two programmes of Ops-2 · 12: rouble cashback points of the bank and coupons of the exchange. */
const PROGRAMMES = [
  {
    name: 'Тинькофф кешбэк',
    wallet: 'Тинькофф',
    symbol: 'ТИНЬКОФФ',
    unit: 'points',
    value: '1',
    currency: 'rub',
    points: '1240',
  },
  {
    name: 'Bybit бонусы',
    wallet: 'Bybit',
    symbol: 'BYBIT',
    unit: 'coupons',
    value: '2.5',
    currency: 'usd',
    points: '2',
  },
] as const

export function seedLoyalty() {
  const at = new Date()
  at.setDate(at.getDate() - 3)
  for (const programme of PROGRAMMES) {
    const wallet = mockStores.find((store) => store.name === programme.wallet && !store.parent_id)
    const store = newStore(programme.name, 'loyalty', '#db2777')
    store.loyalty = {
      points_symbol: programme.symbol,
      point_value: programme.value,
      currency: programme.currency,
      expires_at: null,
      expiring_points: null,
      login: null,
      source_store_ids: wallet ? [wallet.id] : [],
      unit: programme.unit,
    }
    mockStores.push(store)
    mockPrices[programme.symbol] = Number(programme.value) * (priceOf(programme.currency) || 1)
    const input: TransactionInput = {
      type: 'topup',
      common: {
        timestamp: at.toISOString(),
        comment: programme.name,
        categories: ['cashback'],
        flow_id: null,
        contact_id: null,
        price_usd: null,
        fx_override: null,
        price: null,
        price_currency: null,
      },
      store_id: store.id,
      asset: { symbol: programme.symbol, name: programme.name, coingecko_id: null, chain: null, contract: null },
      amount: programme.points,
      fee: null,
      from: null,
      details: STRESS_DETAILS,
    }
    mockTransactions.push(mockTransactionFrom(input, priceOf))
  }
}
