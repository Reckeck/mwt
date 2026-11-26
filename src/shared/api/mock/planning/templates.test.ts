import { describe, expect, it } from 'vitest'

import { mockEnvelopes } from '../catalog'
import { mockStores, newStore } from '../demo'
import { mock } from '../dispatch'
import { automations } from './state'

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

interface Setup {
  rule?: PeriodRule
  amount?: string
  mode?: OnOtherAmount
  percentOnly?: boolean
}

/** A card, a deposit, two envelopes and a salary template over them. */
async function template(name: string, setup: Setup = {}) {
  await mock('list_envelopes')
  const card = newCard(name)
  const deposit = newCard(`${name} deposit`)
  const envelope = async (title: string) =>
    (
      (await mock('create_envelope', {
        input: { store_id: card.id, name: title, asset: RUB, target: null, goal_id: null },
      })) as Envelope
    ).id
  const vacation = await envelope('Vacation')
  const gifts = await envelope('Gifts')
  const rows: DistributionRow[] = setup.percentOnly
    ? [
        { kind: { percent: { pct: '5' } }, target: { envelope: { id: vacation } } },
        { kind: 'remainder', target: 'none' },
      ]
    : [
        { kind: { sum: { amount: '20000' } }, target: { store: { id: deposit.id } } },
        { kind: { percent: { pct: '5' } }, target: { envelope: { id: vacation } } },
        { kind: { sum: { amount: '3000' } }, target: { envelope: { id: gifts } } },
        { kind: 'remainder', target: 'none' },
      ]
  const input: OperationTemplateInput = {
    name,
    kind: 'income',
    store_id: card.id,
    asset: RUB,
    amount: setup.amount ?? '120000',
    categories: ['salary'],
    comment: '',
    period_rule: setup.rule,
    distribution: { rows, on_other_amount: setup.mode ?? 'shrink_remainder' },
  }
  const made = (await mock('create_operation_template', { input })) as OperationTemplate
  return { card, made, input, vacation, gifts }
}
const newCard = (name: string) => {
  const store = newStore(name, 'bank', '#333333')
  mockStores.push(store)
  return store
}

const quickAdd = (templateId: string, timestamp: string, amount?: string) =>
  mock('create_transaction_from_template', {
    input: { template_id: templateId, timestamp, amount },
  }) as Promise<TemplateOperation>
const partsOf = async (tx: Transaction) =>
  (
    (await mock('get_income_distribution', {
      incomeId: (tx as { topup: { meta: TxMeta } }).topup.meta.id,
    })) as IncomeTree
  ).parts.map((entry) => entry.part.amount)
const allocated = (id: string) => Number(mockEnvelopes.find((entry) => entry.id === id)?.allocated)

describe('A4b operation templates (T1-T5)', () => {
  it('quick-adds a salary on 5 Nov with period 16-31 Oct and the A2 result in one write', async () => {
    const { made, vacation, gifts } = await template('Salary A4b', { rule: 'prev_half_second' })
    const result = await quickAdd(made.id, '2026-11-05T09:00:00Z')
    expect(result.ask).toBe(false)
    expect((result.transaction as { topup: { meta: TxMeta } }).topup.meta.period).toEqual({
      kind: 'half',
      start: '2026-10-16',
      end: '2026-10-31',
    })
    expect(await partsOf(result.transaction)).toEqual(['20000', '6000', '3000', '91000'])
    expect(allocated(vacation)).toBe(6000)
    expect(allocated(gifts)).toBe(3000)
  })

  it('refuses a second template with the same name', async () => {
    const { input } = await template('Unique A4b')
    const refused = await refusal(() =>
      mock('create_operation_template', { input: { ...input, name: ' unique a4b ' } }),
    )
    expect(refused?.message).toBe('a template named unique a4b already exists')
  })

  it('refuses to delete a template an automation refers to', async () => {
    const { made } = await template('Used A4b')
    const user = { id: crypto.randomUUID(), name: 'Pay day', operation_template_id: made.id } as Automation
    automations.push(user)
    const refused = await refusal(() => mock('delete_operation_template', { id: made.id }))
    expect(refused?.message).toBe('template Used A4b is used by the automation Pay day; archive it instead')
    automations.splice(automations.indexOf(user), 1)
    await mock('delete_operation_template', { id: made.id })
  })
})

describe('A3 another amount than the rule expects (D9, D11)', () => {
  it('shrink_remainder: 20 000 + 5 925 + 3 000 + 89 575', async () => {
    const { made } = await template('Shrink remainder A3', { mode: 'shrink_remainder' })
    expect(await partsOf((await quickAdd(made.id, '2026-11-05T09:00:00Z', '118500')).transaction)).toEqual([
      '20000',
      '5925',
      '3000',
      '89575',
    ])
  })

  it('shrink_all: the sum rows scale by 118 500 / 120 000 and the total stays 118 500', async () => {
    const { made } = await template('Shrink all A3', { mode: 'shrink_all' })
    const amounts = await partsOf((await quickAdd(made.id, '2026-11-05T09:00:00Z', '118500')).transaction)
    expect(amounts.slice(0, 3)).toEqual(['19750', '5925', '2962.5'])
    expect(amounts.reduce((sum, amount) => sum + Number(amount), 0)).toBe(118500)
  })

  it('ask: the income is saved whole and nothing is distributed', async () => {
    const { made, vacation } = await template('Ask A3', { mode: 'ask' })
    const result = await quickAdd(made.id, '2026-11-05T09:00:00Z', '118500')
    expect(result.ask).toBe(true)
    expect(await partsOf(result.transaction)).toEqual([])
    expect(allocated(vacation)).toBe(0)
  })
})

describe('A4 an advance of 30 000 (D1, D6, D8, T4, T5)', () => {
  it('takes period 1-15 Nov and puts 1 500 into the envelope, 28 500 stays', async () => {
    const { made, vacation } = await template('Advance A4', {
      rule: 'this_half_first',
      amount: '30000',
      percentOnly: true,
    })
    const result = await quickAdd(made.id, '2026-11-20T09:00:00Z')
    expect((result.transaction as { topup: { meta: TxMeta } }).topup.meta.period).toEqual({
      kind: 'half',
      start: '2026-11-01',
      end: '2026-11-15',
    })
    expect(await partsOf(result.transaction)).toEqual(['1500', '28500'])
    expect(allocated(vacation)).toBe(1500)
  })
})
