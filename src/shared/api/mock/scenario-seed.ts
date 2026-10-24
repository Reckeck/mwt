/** Fills the mock once, as the scenario (`?mock=`) asks: see scenario.ts. */

import { mockTransactionFrom } from './ledger'
import { demoTransactions, mockStores, mockTransactions, newStore, priceOf } from './demo'
import { STRESS_DETAILS, STRESS_NO_RATE, STRESS_STORE_NAME, yearInputs } from './demo-year'
import { seedScenarios } from './planning/calc-seed'
import { seedGoal } from './planning/goal-seed'
import { seedSalaryAutomation } from './planning/salary-seed'
import { mockScenario } from './scenario'
import { seedLoyalty } from './loyalty-seed'
import { mockImportRuns, mockKeyStatus } from './exchange-data'

let seeded = false

/** Fill the demo history once, on the first command. */
export function seedMock() {
  if (seeded) return
  seeded = true
  if (mockScenario === 'empty' || mockScenario === 'fresh') {
    // Cash comes with the vault; `empty` shows the list before even that.
    mockStores.splice(mockScenario === 'fresh' ? 1 : 0)
    return
  }
  const bank = mockStores[4]
  const scale = mockScenario === 'stress' ? 40 : 1
  const year = yearInputs(bank.id, new Date(), scale).map((input) => mockTransactionFrom(input, priceOf))
  mockTransactions.push(
    ...year,
    ...demoTransactions(),
    seedThing('FLAT', 'Квартира', 'Покупка квартиры'),
    seedThing('CAR', 'Машина', 'Покупка машины'),
  )
  seedScenarios()
  seedSalaryAutomation(mockTransactions)
  seedGoal()
  seedLoyalty()
  seedExchangeImport()
  if (mockScenario === 'stress') seedStress()
}

/** Bybit connected, with one import behind it (Wallets-2 · 8): 1 trade, 2 transfers, 3 duplicates. */
function seedExchangeImport() {
  const bybit = mockStores.find((store) => store.kind === 'exchange' && !store.parent_id)
  if (!bybit) return
  mockKeyStatus.value = { ...mockKeyStatus.value, configured: true, hint: '…7Q2f' }
  if (bybit.exchange) bybit.exchange.import_interval_sec = 3600
  const at = new Date()
  at.setHours(at.getHours() - 3, 0, 0, 0)
  mockImportRuns.push({
    id: 'mock-run-1',
    store_id: bybit.id,
    at: at.toISOString(),
    source: 'auto',
    fetched: 6,
    staged: 3,
    known: 3,
    filtered: 0,
    confirmed: 3,
    trades: 1,
    transfers: 2,
    reconcile_tx_id: null,
    error: null,
  })
}

/** The flat and the car bought before the year began, so capital has property under it. */
function seedThing(symbol: string, name: string, comment: string): Transaction {
  const owner = mockStores.find((row) => row.kind === 'property' && row.name === name)
  const at = new Date()
  at.setMonth(at.getMonth() - 13)
  const input: TransactionInput = {
    type: 'topup',
    common: { ...PLAIN_COMMON, timestamp: at.toISOString(), comment, categories: ['transfer'] },
    store_id: owner?.id ?? mockStores[0].id,
    asset: { symbol, name, coingecko_id: null, chain: null, contract: null },
    amount: '1',
    fee: null,
    from: null,
    details: STRESS_DETAILS,
  }
  return mockTransactionFrom(input, priceOf)
}

const PLAIN_COMMON: TxCommonInput = {
  timestamp: '',
  comment: '',
  categories: [],
  flow_id: null,
  contact_id: null,
  price_usd: null,
  fx_override: null,
  price: null,
  price_currency: null,
}

/** A wallet with a long name and a balance in a currency the rates do not know. */
function seedStress() {
  const store = newStore(STRESS_STORE_NAME, 'bank', '#14b8a6')
  mockStores.push(store)
  const input: TransactionInput = {
    type: 'topup',
    common: {
      ...PLAIN_COMMON,
      timestamp: new Date().toISOString(),
      comment: 'Остаток в тенге без курса',
      categories: ['transfer'],
    },
    store_id: store.id,
    asset: STRESS_NO_RATE,
    amount: '1250000',
    fee: null,
    from: null,
    details: STRESS_DETAILS,
  }
  mockTransactions.push(mockTransactionFrom(input, priceOf))
  // Two purchases in the same currency: Траты say the total is a lower bound (Currency-1 · 7).
  const now = new Date()
  for (const day of [3, 5]) {
    const at = new Date(now.getFullYear(), now.getMonth(), Math.min(day, now.getDate()), 13)
    const payment: TransactionInput = {
      type: 'payment',
      common: { ...PLAIN_COMMON, timestamp: at.toISOString(), comment: 'Рынок', categories: ['groceries'] },
      store_id: store.id,
      asset: STRESS_NO_RATE,
      amount: '8500',
      fee: null,
      to: { raw: 'Рынок' },
      details: STRESS_DETAILS,
    }
    mockTransactions.push(mockTransactionFrom(payment, priceOf))
  }
}
