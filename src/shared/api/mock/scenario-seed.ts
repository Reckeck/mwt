/** Fills the mock once, as the scenario (`?mock=`) asks: see scenario.ts. */

import { mockTransactionFrom } from './ledger'
import { demoTransactions, mockStores, mockTransactions, newStore, priceOf } from './demo'
import { STRESS_DETAILS, STRESS_NO_RATE, STRESS_STORE_NAME, yearInputs } from './demo-year'
import { seedScenarios } from './planning/calc-seed'
import { mockScenario } from './scenario'

let seeded = false

/** Fill the demo history once, on the first command. */
export function seedMock() {
  if (seeded) return
  seeded = true
  if (mockScenario === 'empty') {
    mockStores.length = 0
    return
  }
  const bank = mockStores[4]
  const scale = mockScenario === 'stress' ? 40 : 1
  const year = yearInputs(bank.id, new Date(), scale).map((input) => mockTransactionFrom(input, priceOf))
  mockTransactions.push(...year, ...demoTransactions(), seedFlat())
  seedScenarios()
  if (mockScenario === 'stress') seedStress()
}

/** The flat bought before the year began, so capital has property under it. */
function seedFlat(): Transaction {
  const flat = mockStores.find((store) => store.kind === 'property')
  const at = new Date()
  at.setMonth(at.getMonth() - 13)
  const input: TransactionInput = {
    type: 'topup',
    common: { ...PLAIN_COMMON, timestamp: at.toISOString(), comment: 'Покупка квартиры', categories: ['transfer'] },
    store_id: flat?.id ?? mockStores[0].id,
    asset: { symbol: 'FLAT', name: 'Квартира', coingecko_id: null, chain: null, contract: null },
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
    common: { ...PLAIN_COMMON, timestamp: new Date().toISOString(), comment: 'Остаток в тенге без курса' },
    store_id: store.id,
    asset: STRESS_NO_RATE,
    amount: '1250000',
    fee: null,
    from: null,
    details: STRESS_DETAILS,
  }
  mockTransactions.push(mockTransactionFrom(input, priceOf))
}
