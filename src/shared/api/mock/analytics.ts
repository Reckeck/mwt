/**
 * Browser-mock half of analytics: reports and the calendar for `pnpm dev`.
 * Rough sums over the mock ledger in USD, enough to lay the screens out;
 * the real reports (rates by date, real terms, past valuations) live only in
 * the backend.
 */

import { txLegs, txMeta } from '@/shared/lib/transactions'

export interface AnalyticsMockContext {
  transactions: Transaction[]
  categories: Category[]
  stores: Store[]
  priceOf: (symbol: string) => number
  currency: string
}

const money = (value: number) => String(Math.round(value * 100) / 100)
const monthKey = (date: Date) => `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`

function monthsOf(filter: ReportFilter, kind: ReportKind): string[] {
  const now = new Date()
  if (kind === 'year') {
    const year = filter.year ?? now.getFullYear()
    return Array.from({ length: 12 }, (_unused, index) => `${year}-${String(index + 1).padStart(2, '0')}`)
  }
  const to = filter.to ? new Date(`${filter.to}-01T12:00`) : new Date(now.getFullYear(), now.getMonth(), 1, 12)
  const from = filter.from ? new Date(`${filter.from}-01T12:00`) : new Date(to.getFullYear(), to.getMonth() - 11, 1, 12)
  const out: string[] = []
  for (const cursor = new Date(from); cursor <= to && out.length < 120; cursor.setMonth(cursor.getMonth() + 1)) {
    out.push(monthKey(cursor))
  }
  return out
}

interface MockFlow {
  month: string
  date: string
  group: CategoryGroup | null
  key: string
  value: number
  incoming: boolean
}

function flows(ctx: AnalyticsMockContext): MockFlow[] {
  return ctx.transactions.flatMap((tx) => {
    const meta = txMeta(tx)
    const key = meta.categories[0] ?? ''
    const group = ctx.categories.find((entry) => entry.key === key)?.group ?? null
    if (!('topup' in tx) && !('payment' in tx)) return []
    const leg = txLegs(tx).find((entry) => entry.role === 'principal')
    if (!leg) return []
    const value = Math.abs(Number(leg.amount)) * (Number(leg.price_usd) || ctx.priceOf(leg.asset.symbol))
    return [
      {
        month: meta.timestamp.slice(0, 7),
        date: meta.timestamp.slice(0, 10),
        group,
        key,
        value,
        incoming: 'topup' in tx,
      },
    ]
  })
}

function meta(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): ReportMeta {
  return { currency: ctx.currency, months, real_terms: filter.real_terms, inflation_rate: null }
}

function cashflow(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): CashflowReport {
  const all = flows(ctx)
  const rows = months.map((month): CashflowRow => {
    const of = (test: (flow: MockFlow) => boolean) =>
      all.filter((flow) => flow.month === month && test(flow)).reduce((sum, flow) => sum + flow.value, 0)
    const income = of((flow) => flow.incoming && flow.group === 'income')
    const fixed = of((flow) => !flow.incoming && flow.group === 'fixed')
    const daily = of((flow) => !flow.incoming && flow.group !== 'fixed' && flow.group !== 'service')
    return {
      month,
      income: money(income),
      fixed: money(fixed),
      daily: money(daily),
      savings_moved: '0',
      budget_in: '0',
      budget_out: '0',
      net: money(income - fixed - daily),
    }
  })
  const sum = (field: keyof CashflowRow) => money(rows.reduce((acc, row) => acc + Number(row[field]), 0))
  return {
    meta: meta(ctx, months, filter),
    rows,
    total: {
      month: '',
      income: sum('income'),
      fixed: sum('fixed'),
      daily: sum('daily'),
      savings_moved: '0',
      budget_in: '0',
      budget_out: '0',
      net: sum('net'),
    },
  }
}

function yearReport(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): YearReport {
  const cash = cashflow(ctx, months, filter)
  const toRow = (row: CashflowRow): YearRow => {
    const income = Number(row.income)
    const fact = income - Number(row.fixed) - Number(row.daily)
    const plan = income * 0.1
    return {
      month: row.month,
      income: row.income,
      fixed: row.fixed,
      daily: row.daily,
      savings_plan: money(plan),
      savings_fact: money(fact),
      deviation: money(fact - plan),
      saved_pct: income ? money((fact / income) * 100) : '0',
      empty: !income && !Number(row.fixed) && !Number(row.daily),
    }
  }
  const rows = cash.rows.map(toRow)
  const filled = rows.filter((row) => !row.empty)
  const byFact = [...filled].sort((left, right) => Number(right.savings_fact) - Number(left.savings_fact))
  const avg = (field: 'income' | 'fixed' | 'daily') =>
    money(filled.reduce((sum, row) => sum + Number(row[field]), 0) / Math.max(1, filled.length))
  return {
    meta: cash.meta,
    year: Number(months[0]?.slice(0, 4)),
    rows,
    total: toRow(cash.total),
    average: toRow({ ...cash.total, income: avg('income'), fixed: avg('fixed'), daily: avg('daily') }),
    best_month: byFact[0]?.month ?? null,
    worst_month: byFact[byFact.length - 1]?.month ?? null,
  }
}

function categories(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): CategoriesReport {
  const all = flows(ctx).filter((flow) => months.includes(flow.month))
  const groups: CategoryGroupBlock[] = []
  for (const group of ['income', 'fixed', 'daily', 'savings'] as CategoryGroup[]) {
    const keys = [...new Set(all.filter((flow) => (flow.group ?? 'daily') === group).map((flow) => flow.key))]
    if (!keys.length) continue
    const rows = keys.map((key): CategoryRow => {
      const cells = months.map((month) =>
        all.filter((flow) => flow.month === month && flow.key === key).reduce((sum, flow) => sum + flow.value, 0),
      )
      const total = cells.reduce((sum, cell) => sum + cell, 0)
      return {
        key,
        group,
        cells: cells.map(money),
        total: money(total),
        average: money(total / months.length),
        share: '0',
        delta_pct: null,
      }
    })
    const total = rows.reduce((sum, row) => sum + Number(row.total), 0)
    for (const row of rows) row.share = money(total ? (Number(row.total) / total) * 100 : 0)
    rows.sort((left, right) => Number(right.total) - Number(left.total))
    groups.push({
      group,
      rows,
      totals: months.map((_month, index) => money(rows.reduce((sum, row) => sum + Number(row.cells[index]), 0))),
      total: money(total),
    })
  }
  const top = groups
    .filter((block) => block.group === 'fixed' || block.group === 'daily')
    .flatMap((block) => block.rows)
    .sort((left, right) => Number(right.total) - Number(left.total))
    .slice(0, 5)
  return { meta: meta(ctx, months, filter), groups, top }
}

function networth(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): NetworthReport {
  const points = months.map((month): NetworthPoint => {
    const value = ctx.transactions
      .filter((tx) => txMeta(tx).timestamp.slice(0, 7) <= month)
      .flatMap((tx) => txLegs(tx))
      .filter((leg) => leg.role === 'principal')
      .reduce((sum, leg) => sum + Number(leg.amount) * ctx.priceOf(leg.asset.symbol), 0)
    return {
      month,
      financial: money(value),
      non_financial: '0',
      receivables: '0',
      liabilities: '0',
      payables: '0',
      net: money(value),
    }
  })
  return { meta: meta(ctx, months, filter), points }
}

export function mockReport(kind: ReportKind, filter: ReportFilter, ctx: AnalyticsMockContext): AnalyticsReport {
  const months = monthsOf(filter, kind)
  switch (kind) {
    case 'cashflow':
      return { cashflow: cashflow(ctx, months, filter) }
    case 'year':
      return { year: yearReport(ctx, months, filter) }
    case 'categories':
      return { categories: categories(ctx, months, filter) }
    case 'networth':
      return { networth: networth(ctx, months, filter) }
    case 'stores':
      return {
        stores: {
          meta: meta(ctx, months, filter),
          rows: ctx.stores.map((store) => ({
            store_id: store.id,
            name: store.name,
            depth: store.parent_id ? 1 : 0,
            kind: store.kind,
            value_start: '0',
            value_end: '0',
            change: '0',
            inflow: '0',
            outflow: '0',
            tx_count: ctx.transactions.filter((tx) => txLegs(tx).some((leg) => leg.store_id === store.id)).length,
          })),
        },
      }
    case 'pnl':
      return {
        pnl: {
          meta: meta(ctx, months, filter),
          rows: months.map((month) => ({ month, realized: '0', fees: '0' })),
          total_realized: '0',
          total_fees: '0',
        },
      }
    case 'portfolio': {
      const worth = networth(ctx, months, filter)
      return {
        portfolio: {
          meta: worth.meta,
          assets: ['USD'],
          points: worth.points.map((point) => ({
            date: `${point.month}-28`,
            total: point.net,
            values: [point.net],
            shares: ['100'],
          })),
        },
      }
    }
    case 'losses':
      return {
        losses: {
          meta: meta(ctx, months, filter),
          rows: months.map((month) => ({ month, fee: '0', slippage: '0', total: '0' })),
          total: { month: '', fee: '0', slippage: '0', total: '0' },
          by_kind: [],
          by_store: [],
          by_contact: [],
          by_flow: [],
          incomplete: 0,
        },
      }
    case 'inflation': {
      const worth = networth(ctx, months, filter)
      return {
        inflation: {
          meta: worth.meta,
          points: worth.points.map((point, index) => {
            const coefficient = 1.03 ** ((months.length - 1 - index) / 12)
            return {
              month: point.month,
              coefficient: money(coefficient),
              nominal_net: point.net,
              real_net: money(Number(point.net) * coefficient),
            }
          }),
          nominal_change_pct: null,
          real_change_pct: null,
        },
      }
    }
  }
}

export function mockCalendar(month: string, ctx: AnalyticsMockContext): CalendarMonth {
  const [year, monthNumber] = month.split('-').map(Number)
  const length = new Date(year, monthNumber, 0).getDate()
  const today = new Date().toISOString().slice(0, 10)
  const all = flows(ctx)
  const allowance = 100
  const days = Array.from({ length }, (_unused, index): CalendarDay => {
    const date = `${month}-${String(index + 1).padStart(2, '0')}`
    const ofDay = all.filter((flow) => flow.date === date)
    const daily = ofDay
      .filter((flow) => !flow.incoming && flow.group !== 'fixed')
      .reduce((sum, flow) => sum + flow.value, 0)
    const spent = ofDay.filter((flow) => !flow.incoming).reduce((sum, flow) => sum + flow.value, 0)
    const status: DayStatus =
      date > today ? 'future' : daily > allowance ? 'over' : daily > allowance * 0.8 ? 'near' : 'under'
    return {
      date,
      income: money(ofDay.filter((flow) => flow.incoming).reduce((sum, flow) => sum + flow.value, 0)),
      spent: money(spent),
      daily: money(daily),
      tx_count: ctx.transactions.filter((tx) => txMeta(tx).timestamp.slice(0, 10) === date).length,
      status,
    }
  })
  return { month, currency: ctx.currency, allowance: String(allowance), days, upcoming: [] }
}
