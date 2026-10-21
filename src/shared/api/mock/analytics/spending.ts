/** The dashboard «Траты» widget for `pnpm dev`: everyday spending by day, no plan base. */

import { flows, money } from './flows'
import type { AnalyticsMockContext } from './flows'

const DAY_MS = 86_400_000
const PERIOD_MONTHS: Record<DashPeriod, number> = { month: 1, quarter: 3, year: 12 }

const isoDate = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`

const compare = (value: number, base: number): Comparison => ({
  value: money(value),
  base: money(base),
  base_kind: 'prev_period',
})

export function mockSpendingOverview(period: DashPeriod, ctx: AnalyticsMockContext): SpendingOverview {
  const now = new Date()
  const today = isoDate(now)
  const span = PERIOD_MONTHS[period]
  const firstMonth =
    period === 'year' ? 0 : period === 'quarter' ? now.getMonth() - (now.getMonth() % 3) : now.getMonth()
  const start = isoDate(new Date(now.getFullYear(), firstMonth, 1))
  const prevStart = isoDate(new Date(now.getFullYear(), firstMonth - span, 1))
  const prevEnd = isoDate(new Date(now.getFullYear(), now.getMonth() - span, now.getDate()))
  const spending = flows(ctx).filter((flow) => !flow.incoming)
  const sum = (from: string, to: string, onlyDaily = false) =>
    spending
      .filter((flow) => flow.date >= from && flow.date <= to && (!onlyDaily || flow.group !== 'fixed'))
      .reduce((total, flow) => total + flow.value, 0)
  const dayOffset = (offset: number) => isoDate(new Date(now.getTime() - offset * DAY_MS))
  const allowance = 100
  const week = Array.from({ length: 7 }, (_unused, index): SpendingDay => {
    const date = dayOffset(6 - index)
    const daily = sum(date, date, true)
    return { date, daily: money(daily), over: daily > allowance }
  })
  const series: Amount[] = []
  if (period === 'year') {
    for (let month = 0; month <= now.getMonth(); month++)
      series.push(
        money(sum(isoDate(new Date(now.getFullYear(), month, 1)), isoDate(new Date(now.getFullYear(), month + 1, 0)))),
      )
  } else {
    const step = period === 'month' ? 1 : 7
    for (
      let cursor = new Date(`${start}T12:00`);
      isoDate(cursor) <= today;
      cursor = new Date(cursor.getTime() + step * DAY_MS)
    )
      series.push(money(sum(isoDate(cursor), isoDate(new Date(cursor.getTime() + (step - 1) * DAY_MS)))))
  }
  return {
    currency: ctx.currency,
    period,
    start,
    spent: compare(sum(start, today), sum(prevStart, prevEnd)),
    daily: compare(sum(start, today, true), sum(prevStart, prevEnd, true)),
    series,
    week,
    week_total: compare(sum(dayOffset(6), today, true), sum(dayOffset(13), dayOffset(7), true)),
    allowance: String(allowance),
  }
}
