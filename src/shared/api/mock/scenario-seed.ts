/** Fills the mock once, as the scenario (`?mock=`) asks: see scenario.ts. */

import { mockTransactionFrom } from './ledger'
import { demoTransactions, mockStores, mockTransactions, newStore, priceOf } from './demo'
import { STRESS_DETAILS, STRESS_NO_RATE, STRESS_STORE_NAME, yearInputs } from './demo-year'
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
  mockTransactions.push(...year, ...demoTransactions())
  if (mockScenario === 'stress') seedStress()
}

/** A wallet with a long name and a balance in a currency the rates do not know. */
function seedStress() {
  const store = newStore(STRESS_STORE_NAME, 'bank', '#14b8a6')
  mockStores.push(store)
  const input: TransactionInput = {
    type: 'topup',
    common: {
      timestamp: new Date().toISOString(),
      comment: 'Остаток в тенге без курса',
      categories: [],
      flow_id: null,
      contact_id: null,
      price_usd: null,
      fx_override: null,
      price: null,
      price_currency: null,
    },
    store_id: store.id,
    asset: STRESS_NO_RATE,
    amount: '1250000',
    fee: null,
    from: null,
    details: STRESS_DETAILS,
  }
  mockTransactions.push(mockTransactionFrom(input, priceOf))
}
