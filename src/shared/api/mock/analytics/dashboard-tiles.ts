/** The dashboard «Бюджет дня», «Итоги месяца» and «Чистый капитал» tiles for `pnpm dev`. */

import { flows, money } from './flows'
import type { AnalyticsMockContext } from './flows'
import { mockNetWorth } from '../market'
import { ledgerWorth } from './worth-split'

const isoDate = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`

const compare = (value: number, base: number | null): Comparison => ({
  value: money(value),
  base: base === null ? null : money(base),
  base_kind: 'prev_period',
})

export function mockMonthPace(ctx: AnalyticsMockContext, savingsRate: number): MonthPace {
  const now = new Date()
  const day = now.getDate()
  const days = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate()
  const start = isoDate(new Date(now.getFullYear(), now.getMonth(), 1))
  const today = isoDate(now)
  const prevStart = isoDate(new Date(now.getFullYear(), now.getMonth() - 1, 1))
  const prevEnd = isoDate(new Date(now.getFullYear(), now.getMonth() - 1, Math.min(day, days)))
  const all = flows(ctx)
  const within = (from: string, to: string) => all.filter((flow) => flow.date >= from && flow.date <= to)
  const sum = (list: typeof all, incoming: boolean) =>
    list.filter((flow) => flow.incoming === incoming).reduce((total, flow) => total + flow.value, 0)
  const current = within(start, today)
  const previous = within(prevStart, prevEnd)
  const hasHistory = all.some((flow) => flow.date < start)
  const income = sum(current, true)
  const spent = sum(current, false)
  const prevIncome = hasHistory ? sum(previous, true) : null
  const prevSpent = hasHistory ? sum(previous, false) : null
  const byCategory = (list: typeof all) => {
    const out = new Map<string, number>()
    for (const flow of list) if (!flow.incoming) out.set(flow.key, (out.get(flow.key) ?? 0) + flow.value)
    return out
  }
  const nowBy = byCategory(current)
  const prevBy = byCategory(previous)
  const growth: CategoryGrowth[] = hasHistory
    ? [...nowBy.entries()]
        .map(([category, value]) => ({ category, value, base: prevBy.get(category) ?? 0 }))
        .filter((row) => row.value > row.base && row.category)
        .sort((left, right) => right.value - right.base - (left.value - left.base))
        .slice(0, 3)
        .map((row) => ({ category: row.category, value: money(row.value), base: money(row.base) }))
    : []
  return {
    currency: ctx.currency,
    month: start.slice(0, 7),
    day,
    days,
    income: compare(income, prevIncome),
    spent: compare(spent, prevSpent),
    saved: compare(income - spent, prevIncome === null || prevSpent === null ? null : prevIncome - prevSpent),
    savings_plan: money(income * savingsRate),
    spend_plan_to_date: income > 0 ? money(((income - income * savingsRate) * day) / days) : null,
    forecast_spent: money((spent * days) / day),
    growth,
  }
}

/**
 * Like the core: the figure now against the end of the month before the period
 * (the capital report's previous point), so the tile and the report give one change.
 */
export function mockNetWorthTrend(period: DashPeriod, ctx: AnalyticsMockContext): NetWorthTrend {
  const now = new Date()
  const total = Number(mockNetWorth().net)
  const steps = period === 'month' ? now.getDate() : period === 'quarter' ? 13 : now.getMonth() + 1
  const firstMonth =
    period === 'year' ? 0 : period === 'quarter' ? now.getMonth() - (now.getMonth() % 3) : now.getMonth()
  const before = new Date(now.getFullYear(), firstMonth - 1, 1)
  const key = `${before.getFullYear()}-${String(before.getMonth() + 1).padStart(2, '0')}`
  const opening = ledgerWorth(ctx, key)
  const base = opening.financial + opening.non_financial - Math.max(0, opening.liabilities)
  const series = Array.from({ length: steps }, (_unused, index) => money(base + ((total - base) * (index + 1)) / steps))
  return {
    currency: ctx.currency,
    period,
    start: isoDate(new Date(now.getFullYear(), firstMonth, 1)),
    change: compare(total, base),
    series,
  }
}
