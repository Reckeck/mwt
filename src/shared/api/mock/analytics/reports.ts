/** Cash flow, year, category and net worth reports. */

import { txLegs, txMeta } from '@/shared/lib/transactions'
import { flows, meta, money } from './flows'
import type { AnalyticsMockContext, MockFlow } from './flows'

export function cashflow(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): CashflowReport {
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

export function yearReport(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): YearReport {
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

export function categories(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): CategoriesReport {
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

export function networth(ctx: AnalyticsMockContext, months: string[], filter: ReportFilter): NetworthReport {
  // Like the core: every leg a store holds (fees too), split by the store's kind.
  const kinds = new Map(ctx.stores.map((store) => [store.id, store.kind]))
  const scope = filter.scope.store_ids
  const points = months.map((month): NetworthPoint => {
    const split = { financial: 0, crypto: 0, non_financial: 0, liabilities: 0 }
    for (const tx of ctx.transactions) {
      if (txMeta(tx).timestamp.slice(0, 7) > month) continue
      for (const leg of txLegs(tx)) {
        if (leg.asset.contract?.startsWith('perp:') || (scope.length && !scope.includes(leg.store_id))) continue
        const value = Number(leg.amount) * ctx.priceOf(leg.asset.symbol)
        const kind = kinds.get(leg.store_id)
        if (kind === 'property' || kind === 'loyalty') split.non_financial += value
        else if (kind === 'liability') split.liabilities -= value
        else {
          split.financial += value
          if (kind === 'crypto' || kind === 'exchange') split.crypto += value
        }
      }
    }
    const net = split.financial + split.non_financial - split.liabilities
    return {
      month,
      financial: money(split.financial),
      crypto: money(split.crypto),
      non_financial: money(split.non_financial),
      receivables: '0',
      liabilities: money(Math.max(0, split.liabilities)),
      payables: '0',
      net: money(net),
    }
  })
  return { meta: meta(ctx, months, filter), points, has_data: points.some((point) => Number(point.net) !== 0) }
}
