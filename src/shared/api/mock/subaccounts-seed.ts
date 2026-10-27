/** Sub-accounts for the demo: a bank's card and savings, an exchange's funding and earn accounts. */

import { mockTransactionFrom } from './ledger'
import { mockMarketBinding, mockStores, mockTransactions, newStore, priceOf } from './demo'
import { STRESS_DETAILS } from './demo-year'

type Coin = { symbol: string; name: string; chain: Chain | null }
const RUB: Coin = { symbol: 'RUB', name: 'Рубль', chain: 'fiat' }
const USDT: Coin = { symbol: 'USDT', name: 'Tether', chain: null }

function topup(storeId: string, coin: Coin, amount: string, comment: string): Transaction {
  const input: TransactionInput = {
    type: 'topup',
    common: {
      timestamp: new Date(Date.now() - 40 * 86_400_000).toISOString(),
      comment,
      categories: ['transfer'],
      flow_id: null,
      contact_id: null,
      price_usd: null,
      fx_override: null,
      price: null,
      price_currency: null,
    },
    store_id: storeId,
    asset: { symbol: coin.symbol, name: coin.name, coingecko_id: null, chain: coin.chain, contract: null },
    amount,
    fee: null,
    from: null,
    details: STRESS_DETAILS,
  }
  return mockTransactionFrom(input, priceOf)
}

function child(parent: Store, name: string, subType: SubType, coin: Coin, amount: string) {
  const store: Store = {
    ...newStore(name, parent.kind, parent.color, parent.id),
    sub_type: subType,
    binding: mockMarketBinding(subType),
    exchange: null,
  }
  mockStores.push(store)
  mockTransactions.push(topup(store.id, coin, amount, name))
}

/** The bank gets «Карта RUB» and «Накопительный», the exchange its funding and earn accounts. */
export function seedSubAccounts() {
  const bank = mockStores.find((store) => store.name === 'Тинькофф')
  if (bank) {
    child(bank, 'Карта RUB', 'card', RUB, '52000')
    child(bank, 'Накопительный', 'savings', RUB, '5000')
  }
  const bybit = mockStores.find((store) => store.kind === 'exchange' && !store.parent_id)
  if (bybit) {
    child(bybit, 'Финансовый счёт', 'funding', USDT, '500')
    child(bybit, 'Инвестиционные продукты', 'earn', USDT, '120')
  }
}
