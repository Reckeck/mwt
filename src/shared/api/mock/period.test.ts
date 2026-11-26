import { describe, expect, it } from 'vitest'

import { mockReport } from './analytics/entry'
import { mockCategories } from './ledger'
import { mockTransactionFrom } from './ledger'
import { mockStores, newStore } from './demo'
import { mock } from './dispatch'
import { periodOfRule } from './period'

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

const month = (from: string, to: string): OpPeriod => ({ kind: 'range', start: from, end: to })

describe('A1 the period of a salary (P1-P4, R2, D2)', () => {
  const salary = mockTransactionFrom(
    {
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
        period: { kind: 'half', start: '2026-10-16', end: '2026-10-31' },
      },
      store_id: 'card',
      asset: RUB,
      amount: '120000',
      fee: null,
      details: {} as KindDetails,
      from: null,
      parts: [],
    } as TransactionInput,
    () => 1,
  )
  const context = { transactions: [salary], categories: mockCategories, stores: [], priceOf: () => 1, currency: 'RUB' }
  const incomeOf = (basis: 'period' | 'money_date') => {
    const filter = { scope: { stores: [] }, from: '2026-10', to: '2026-11', basis } as unknown as ReportFilter
    const report = mockReport('cashflow', filter, context)
    if (!('cashflow' in report)) throw new Error('no cashflow')
    return report.cashflow.rows.map((row) => [row.month, row.income])
  }

  it('resolves prev_half_second on 5 Nov to 16-31 Oct', async () => {
    const period = (await mock('resolve_period_rule', {
      rule: 'prev_half_second',
      moneyDate: '2026-11-05',
    })) as OpPeriod
    expect(period).toEqual({ kind: 'half', start: '2026-10-16', end: '2026-10-31' })
    expect(periodOfRule('prev_half_second', '2026-11-05')).toEqual(period)
  })

  it('counts the income in October by the period and in November by the money date', () => {
    expect(incomeOf('period')).toEqual([
      ['2026-10', '120000'],
      ['2026-11', '0'],
    ])
    expect(incomeOf('money_date')).toEqual([
      ['2026-10', '0'],
      ['2026-11', '120000'],
    ])
  })
})

describe('A6 the spread of an amount over months (P5-P8)', () => {
  const spread = (mode: SpreadMode, period: OpPeriod, shares: MonthShare[] = []) =>
    mock('preview_spread', { period, amount: '100', asset: RUB, mode, shares }) as Promise<MonthShare[]>
  const amounts = (shares: MonthShare[]) => shares.map((share) => share.amount)

  it('cuts an even spread 33.33 + 33.33 + 33.34', async () => {
    expect(amounts(await spread('even', month('2026-10-01', '2026-12-31')))).toEqual(['33.33', '33.33', '33.34'])
  })

  it('weights by days for 16 Oct - 15 Dec (16 + 30 + 15) and sums to 100 exactly', async () => {
    const shares = await spread('by_days', month('2026-10-16', '2026-12-15'))
    expect(shares.map((share) => share.month)).toEqual(['2026-10', '2026-11', '2026-12'])
    expect(shares.reduce((sum, share) => sum + Math.round(Number(share.amount) * 100), 0)).toBe(10000)
  })

  it('keeps a manual spread 10 + 20 + 70 and refuses 10 + 20 + 60', async () => {
    const shares = (list: string[]) => list.map((amount, index) => ({ month: `2026-1${index}`, amount }))
    const period = month('2026-10-01', '2026-12-31')
    expect(amounts(await spread('manual', period, shares(['10', '20', '70'])))).toEqual(['10', '20', '70'])
    const refused = await refusal(() => spread('manual', period, shares(['10', '20', '60'])))
    expect(refused?.code).toBe('INVALID_INPUT')
    expect(refused?.message).toBe('the shares do not add up to the amount')
  })
})

describe('A8 the month summary reads money date or period (R2, T29)', () => {
  const card = newStore('Card A8', 'bank', '#444444')
  mockStores.push(card)
  const add = (type: 'topup' | 'payment', amount: string, timestamp: string, category: string, period?: OpPeriod) =>
    mock('add_transaction', {
      input: {
        type,
        common: {
          timestamp,
          comment: '',
          categories: [category],
          flow_id: null,
          contact_id: null,
          price_usd: null,
          fx_override: null,
          price: null,
          price_currency: null,
          period,
        },
        store_id: card.id,
        asset: RUB,
        amount,
        fee: null,
        details: {},
        ...(type === 'topup' ? { from: null, parts: [] } : { to: null }),
      },
    })
  const summary = (monthKey: string, basis: 'period' | 'money_date') =>
    mock('get_month_summary', { month: monthKey, basis }) as Promise<MonthSummary>

  it('counts a 30 000 advance and a 120 000 salary by the money date or by their periods', async () => {
    const income = async (monthKey: string, basis: 'period' | 'money_date') =>
      Number((await summary(monthKey, basis)).income)
    const before = {
      novDate: await income('2026-11', 'money_date'),
      novPeriod: await income('2026-11', 'period'),
      octPeriod: await income('2026-10', 'period'),
    }
    await add('topup', '30000', '2026-11-20T09:00:00Z', 'salary', {
      kind: 'half',
      start: '2026-11-01',
      end: '2026-11-15',
    })
    await add('topup', '120000', '2026-11-05T09:00:00Z', 'salary', {
      kind: 'half',
      start: '2026-10-16',
      end: '2026-10-31',
    })
    await add('payment', '10000', '2026-11-10T09:00:00Z', 'food')
    // The amounts are in roubles, the summary in the budget currency: compare in units of the 30 000.
    const unit = (await income('2026-11', 'period')) - before.novPeriod
    expect(unit).toBeGreaterThan(0)
    expect((await income('2026-11', 'money_date')) - before.novDate).toBeCloseTo(unit * 5, 1)
    expect((await income('2026-10', 'period')) - before.octPeriod).toBeCloseTo(unit * 4, 1)
    const novByDate = await summary('2026-11', 'money_date')
    const novByPeriod = await summary('2026-11', 'period')
    expect(novByPeriod.daily).toBe(novByDate.daily)
    expect(novByPeriod.fixed).toBe(novByDate.fixed)
    expect(Number(novByPeriod.daily)).toBeGreaterThan(0)
  })
})

describe('R3, R4 a budget status follows the basis only for a Month budget (T53)', () => {
  const card = newStore('Card T53', 'bank', '#555555')
  mockStores.push(card)
  const status = async (period: 'day' | 'week' | 'month', basis: 'period' | 'money_date') => {
    const created = (await mock('create_budget', {
      input: { active: true, target: 'total', period, limit: '1000000', currency: 'USD', warn: false },
    })) as Budget
    const all = (await mock('get_budget_status', { basis })) as BudgetStatus[]
    const found = all.find((entry) => entry.budget.id === created.id)
    await mock('delete_budget', { id: created.id })
    return Number(found?.spent)
  }

  it('counts an operation for last month by its money date in Day and Week budgets, by basis in Month', async () => {
    const today = new Date()
    const previous = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth() - 1, 1))
    const start = previous.toISOString().slice(0, 10)
    const end = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), 0)).toISOString().slice(0, 10)
    const baseline = {
      day: await status('day', 'period'),
      week: await status('week', 'period'),
      monthPeriod: await status('month', 'period'),
      monthDate: await status('month', 'money_date'),
    }
    await mock('add_transaction', {
      input: {
        type: 'payment',
        common: {
          timestamp: today.toISOString(),
          comment: '',
          categories: ['food'],
          flow_id: null,
          contact_id: null,
          price_usd: null,
          fx_override: null,
          price: null,
          price_currency: null,
          period: { kind: 'range', start, end },
        },
        store_id: card.id,
        asset: RUB,
        amount: '5000',
        fee: null,
        details: {},
        to: null,
      },
    })
    for (const basis of ['period', 'money_date'] as const) {
      expect(await status('day', basis)).toBeGreaterThan(baseline.day)
      expect(await status('week', basis)).toBeGreaterThan(baseline.week)
    }
    expect(await status('month', 'period')).toBe(baseline.monthPeriod)
    expect(await status('month', 'money_date')).toBeGreaterThan(baseline.monthDate)
  })
})
