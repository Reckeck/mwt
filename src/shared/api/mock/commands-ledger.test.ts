import { describe, expect, it } from 'vitest'

import { mock } from './dispatch'
import { mockStores, mockTransactions, newStore } from './demo'

const RUB: AssetIdent = { symbol: 'RUB', name: 'Russian Ruble', coingecko_id: null, chain: null, contract: null }
/** The mock may refuse by throwing or by rejecting: both come back as the error. */
const refusal = async (call: () => Promise<unknown>) => {
  try {
    await call()
    return null
  } catch (error) {
    return error as { code: string; message: string }
  }
}

const income = (storeId: string, amount: string, parts: ReceivedPartInput[]) => ({
  input: {
    type: 'topup',
    common: {
      timestamp: '2026-11-05T09:00:00Z',
      comment: 'salary',
      categories: ['salary'],
      flow_id: null,
      contact_id: null,
      price_usd: null,
      fx_override: null,
      price: null,
      price_currency: null,
    },
    store_id: storeId,
    asset: RUB,
    amount,
    fee: null,
    details: {},
    from: null,
    parts,
  },
})

describe('A5 an income in parts (S1-S5)', () => {
  const tinkoff = newStore('Тинькофф A5', 'bank', '#facc15')
  const cash = newStore('Наличные A5', 'cash', '#16a34a')
  mockStores.push(tinkoff, cash)
  const parts = (second: string): ReceivedPartInput[] => [
    { store_id: tinkoff.id, amount: '100000' },
    { store_id: cash.id, amount: second, timestamp: '2026-11-07T10:00:00Z' },
  ]

  it('stores 100 000 + 20 000 as one income of 120 000', async () => {
    await mock('list_envelopes')
    const before = mockTransactions.length
    const transfers = mockTransactions.filter((tx) => 'transfer' in tx).length
    const stored = (await mock('add_transaction', income(tinkoff.id, '120000', parts('20000')))) as Transaction
    expect(mockTransactions.length).toBe(before + 1)
    if (!('topup' in stored)) throw new Error('not an income')
    expect(stored.topup.amount).toBe('120000')
    expect(stored.topup.parts?.map((part) => [part.store_id, part.amount])).toEqual([
      [tinkoff.id, '100000'],
      [cash.id, '20000'],
    ])
    expect(stored.topup.parts?.[1].timestamp).toBe('2026-11-07T10:00:00Z')
    expect(mockTransactions.filter((tx) => 'transfer' in tx).length).toBe(transfers)
  })

  it('refuses 100 000 + 19 999 and a single part, and writes nothing', async () => {
    await mock('list_envelopes')
    const before = mockTransactions.length
    const short = await refusal(() => mock('add_transaction', income(tinkoff.id, '120000', parts('19999'))))
    expect(short?.message).toBe('the parts must add up to the amount')
    const single = await refusal(() =>
      mock('add_transaction', income(tinkoff.id, '120000', [{ store_id: tinkoff.id, amount: '120000' }])),
    )
    expect(single?.message).toBe('an income in parts needs at least two parts')
    expect(mockTransactions.length).toBe(before)
  })
})
