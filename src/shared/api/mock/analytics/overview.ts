/** The analytics overview and the insights of a report, roughly as core builds them. */

import { money, monthsOf } from './flows'
import type { AnalyticsMockContext } from './flows'
import { cashflow, categories, networth, yearReport } from './reports'

const fact = (code: InsightCode, fields: Partial<Insight> = {}): Insight => ({
  code,
  amount: null,
  pct: null,
  subject: null,
  ...fields,
})

const pctOf = (part: number, whole: number) => money(Math.round((part / Math.abs(whole)) * 1000) / 10)

function balance(income: number, spent: number): Insight[] {
  if (!income && !spent) return []
  if (!income) return [fact('no_income', { amount: money(spent) })]
  const kept = income - spent
  return [fact(kept < 0 ? 'overspent' : 'saved', { amount: money(Math.abs(kept)), pct: pctOf(Math.abs(kept), income) })]
}

function wealth(first: number, last: number): Insight {
  const change = last - first
  if (first ? Math.abs(change / first) < 0.01 : !change) return fact('wealth_flat', { amount: money(last) })
  return fact(change < 0 ? 'wealth_down' : 'wealth_up', {
    amount: money(Math.abs(change)),
    pct: first ? pctOf(Math.abs(change), first) : null,
  })
}

/** Facts of a mock report; only the kinds the mock fills with numbers. */
export function mockInsights(report: AnalyticsReport): Insight[] {
  if ('cashflow' in report) {
    const total = report.cashflow.total
    return balance(Number(total.income), Number(total.fixed) + Number(total.daily))
  }
  if ('year' in report) {
    const total = report.year.total
    return balance(Number(total.income), Number(total.fixed) + Number(total.daily))
  }
  if ('categories' in report) {
    const top = report.categories.top[0]
    return top ? [fact('top_category', { subject: top.key, amount: top.total })] : []
  }
  if ('networth' in report) {
    const points = report.networth.points
    return points.length ? [wealth(Number(points[0].net), Number(points[points.length - 1].net))] : []
  }
  return []
}

export function mockOverview(filter: ReportFilter, ctx: AnalyticsMockContext): ReportOverview {
  const month = monthsOf(filter, 'cashflow').at(-1) ?? ''
  const [year, monthNumber] = month.split('-').map(Number)
  const previous = new Date(year, monthNumber - 2, 1, 12)
  const previousMonth = `${previous.getFullYear()}-${String(previous.getMonth() + 1).padStart(2, '0')}`
  const pair = [previousMonth, month]

  const cash = cashflow(ctx, pair, filter)
  const [before, current] = cash.rows
  const spent = Number(current.fixed) + Number(current.daily)
  const previousSpent = Number(before.fixed) + Number(before.daily)
  const top = categories(ctx, [month], filter).top[0]
  const flowInsights = balance(Number(current.income), spent)
  if (previousSpent && spent !== previousSpent) {
    flowInsights.push(
      fact(spent < previousSpent ? 'spent_down' : 'spent_up', {
        pct: pctOf(Math.abs(spent - previousSpent), previousSpent),
      }),
    )
  }
  if (top) flowInsights.push(fact('top_category', { subject: top.key, amount: top.total }))

  const worth = networth(ctx, pair, filter).points
  const [first, last] = [Number(worth[0].net), Number(worth[1].net)]

  const yearly = yearReport(ctx, monthsOf({ ...filter, year }, 'year'), filter)
  return {
    currency: ctx.currency,
    month,
    previous_month: previousMonth,
    year,
    flow: {
      income: current.income,
      spent: money(spent),
      net: current.net,
      previous_spent: money(previousSpent),
      top_category: top?.key ?? null,
      has_data: Boolean(Number(current.income) || spent || Number(before.income) || previousSpent),
      insights: flowInsights,
    },
    wealth: { net: money(last), change: money(last - first), has_data: last !== 0, insights: [wealth(first, last)] },
    investments: { realized: '0', fees: '0', losses: '0', has_data: false, insights: [] },
    savings: {
      saved: yearly.total.savings_fact,
      saved_pct: yearly.total.saved_pct,
      best_month: yearly.best_month,
      worst_month: yearly.worst_month,
      has_data: yearly.rows.some((row) => !row.empty),
      insights: mockInsights({ year: yearly }),
    },
  }
}
