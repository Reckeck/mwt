import { describe, expect, it } from 'vitest'

import { mockEnvelopes } from '../catalog'
import { mockNotices, mockStores, mockTransactions, newStore } from '../demo'
import { runs as workflowRuns, waits } from '../workflows/state'
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
  over?: boolean
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
  const rows: DistributionRow[] = setup.over
    ? [
        { kind: { sum: { amount: '130000' } }, target: { store: { id: deposit.id } } },
        { kind: 'remainder', target: 'none' },
      ]
    : setup.percentOnly
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

  it('refuses a distribution on an expense template', async () => {
    const { input } = await template('Expense A4b')
    const refused = await refusal(() =>
      mock('create_operation_template', { input: { ...input, name: 'Expense dist A4b', kind: 'expense' } }),
    )
    expect(refused?.message).toBe('only an income template can carry a distribution')
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

describe('D12/Q11, D9/Q12 an automation run books like the core', () => {
  async function automationOf(name: string, setup: Setup) {
    const { made, card, input } = await template(name, setup)
    const created = (await mock('create_automation', {
      input: {
        name,
        active: true,
        schedule: { monthly: { day: 5 } },
        starts_at: '2026-11-05T09:00:00Z',
        operation_template_id: made.id,
        template: {
          type: 'topup',
          common: {
            timestamp: '2026-11-05T09:00:00Z',
            comment: '',
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
          amount: input.amount ?? '120000',
          fee: null,
          details: {},
          from: null,
          parts: [],
        },
      },
    })) as Automation
    return created
  }

  it('over: the income is booked alone and a notice says it is not distributed', async () => {
    const automation = await automationOf('Over D12', { over: true })
    const before = mockTransactions.length
    const tx = (await mock('run_automation', { id: automation.id })) as Transaction
    expect(mockTransactions.some((stored) => stored === tx || JSON.stringify(stored) === JSON.stringify(tx))).toBe(true)
    expect(mockTransactions.length).toBe(before + 1)
    expect(await partsOf(tx)).toEqual([])
    expect(mockNotices.some((notice) => notice.key.startsWith('automation-undistributed:'))).toBe(true)
  })

  it('ask: the run leaves a question and confirm_distribution books and answers it', async () => {
    const automation = await automationOf('Ask D9', { mode: 'ask' })
    const before = mockTransactions.length
    await mock('run_automation', { id: automation.id })
    expect(mockTransactions.length).toBe(before)
    const asking = workflowRuns.find((run) => run.workflow_id === automation.id)
    expect(asking && typeof asking.outcome === 'object' && 'asking' in asking.outcome).toBe(true)
    if (!asking) throw new Error('no run')
    expect(waits.some((wait) => wait.run_id === asking.id)).toBe(true)
    const booked = (await mock('confirm_distribution', {
      input: { run_id: asking.id, amount: '120000', mode: 'shrink_remainder' },
    })) as Transaction
    expect(await partsOf(booked)).toEqual(['20000', '6000', '3000', '91000'])
    expect(waits.some((wait) => wait.run_id === asking.id)).toBe(false)
    expect(asking.outcome).toBe('done')
  })

  it('confirm without a mode on a rule that still asks is refused with the core text', async () => {
    const automation = await automationOf('Still asks D9', { mode: 'ask' })
    await mock('run_automation', { id: automation.id })
    const asking = workflowRuns.find((run) => run.workflow_id === automation.id)
    if (!asking) throw new Error('no run')
    const refused = await refusal(() =>
      mock('confirm_distribution', { input: { run_id: asking.id, amount: '120000' } }),
    )
    expect(refused?.message).toBe('the rule still asks: choose how to distribute')
  })
})
