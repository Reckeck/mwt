import { describe, expect, it } from 'vitest'

import { mockEnvelopes } from '../catalog'
import { mockStores, mockTransactions, newStore } from '../demo'
import { mock } from '../dispatch'

const RUB: AssetRef = {
  symbol: 'RUB',
  name: 'Russian Ruble',
  coingecko_id: null,
  chain: null,
  contract: null,
  quote: 'RUB',
}

async function refusal(call: () => Promise<unknown>) {
  try {
    await call()
    return null
  } catch (error) {
    return error as { code: string; message: string }
  }
}

/** A 120 000 salary on a card, a deposit and two envelopes: the Rust `fixture()` of the distribution tests. */
async function fixture() {
  await mock('list_envelopes')
  const card = newStore('Card', 'bank', '#111111')
  const deposit = newStore('Cushion', 'bank', '#222222')
  mockStores.push(card, deposit)
  const envelope = async (name: string) =>
    (
      (await mock('create_envelope', {
        input: { store_id: card.id, name, asset: RUB, target: null, goal_id: null },
      })) as Envelope
    ).id
  const vacation = await envelope('Vacation')
  const gifts = await envelope('Gifts')
  const salary = (await mock('add_transaction', {
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
      store_id: card.id,
      asset: RUB,
      amount: '120000',
      fee: null,
      details: {},
      from: null,
      parts: [],
    },
  })) as TopupTransaction
  return { card, deposit, vacation, gifts, incomeId: salary.topup.meta.id }
}
type TopupTransaction = Extract<Transaction, { topup: unknown }>
type Fixture = Awaited<ReturnType<typeof fixture>>

const rows = (parts: Fixture, deposit = '20000'): DistributionRow[] => [
  { kind: { sum: { amount: deposit } }, target: { store: { id: parts.deposit.id } } },
  { kind: { percent: { pct: '5' } }, target: { envelope: { id: parts.vacation } } },
  { kind: { sum: { amount: '3000' } }, target: { envelope: { id: parts.gifts } } },
  { kind: 'remainder', target: 'none' },
]
const allocated = (id: string) => Number(mockEnvelopes.find((entry) => entry.id === id)?.allocated)
const transfersOf = () => mockTransactions.filter((tx) => 'transfer' in tx).length

describe('A2 a distribution of a 120 000 income (S6-S9)', () => {
  it('resolves the rows to 20 000 / 6 000 / 3 000 / 91 000 and books the effects', async () => {
    const parts = await fixture()
    const transfers = transfersOf()
    await mock('distribute_income', { input: { income: { source: 'stored', id: parts.incomeId }, rows: rows(parts) } })
    const stored = (await mock('get_income_distribution', { incomeId: parts.incomeId })) as IncomeTree
    const list = stored.parts.map((entry) => entry.part)
    expect(list.map((part) => part.amount)).toEqual(['20000', '6000', '3000', '91000'])
    expect(list.map((part) => part.status)).toEqual(['done', 'done', 'done', 'done'])
    expect(list.map((part) => Boolean(part.effect))).toEqual([true, true, true, false])
    expect(list.reduce((sum, part) => sum + Number(part.amount), 0)).toBe(120000)
    expect(transfersOf()).toBe(transfers + 1)
    expect(allocated(parts.vacation)).toBe(6000)
    expect(allocated(parts.gifts)).toBe(3000)
  })

  it('refuses an over-allocation (130 000 + remainder) and writes nothing', async () => {
    const parts = await fixture()
    const transfers = transfersOf()
    const refused = await refusal(() =>
      mock('distribute_income', {
        input: { income: { source: 'stored', id: parts.incomeId }, rows: rows(parts, '130000') },
      }),
    )
    expect(refused?.code).toBe('INVALID_INPUT')
    expect(transfersOf()).toBe(transfers)
    expect(allocated(parts.vacation)).toBe(0)
    const stored = (await mock('get_income_distribution', { incomeId: parts.incomeId })) as IncomeTree
    expect(stored.parts).toEqual([])
  })
})

describe('A2b a distribution is edited and undone (S11, V1-V4)', () => {
  it('rewrites 20 000 as 25 000 and undo removes every effect', async () => {
    const parts = await fixture()
    const transfers = transfersOf()
    await mock('distribute_income', { input: { income: { source: 'stored', id: parts.incomeId }, rows: rows(parts) } })
    await mock('update_distribution', { input: { income_id: parts.incomeId, rows: rows(parts, '25000') } })
    const edited = (await mock('get_income_distribution', { incomeId: parts.incomeId })) as IncomeTree
    expect(edited.parts.map((entry) => entry.part.amount)).toEqual(['25000', '6000', '3000', '86000'])
    expect(transfersOf()).toBe(transfers + 1)
    await mock('undo_distribution', { incomeId: parts.incomeId })
    expect(transfersOf()).toBe(transfers)
    expect(allocated(parts.vacation)).toBe(0)
    expect(allocated(parts.gifts)).toBe(0)
  })
})
