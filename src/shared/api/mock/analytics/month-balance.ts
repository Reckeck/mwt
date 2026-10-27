/** The month's balance: what each month left and what carried over (the backend's `month_balance`). */

import { mockCalendar } from './entry'
import { cashflow } from './reports'
import { flows, monthsOf, money } from './flows'
import type { AnalyticsMockContext } from './flows'

/** `YYYY-MM` months from `from` through `to`. */
function span(from: string, to: string): string[] {
  const out: string[] = []
  let [year, month] = from.split('-').map(Number)
  for (let guard = 0; guard < 240; guard++) {
    const key = `${year}-${String(month).padStart(2, '0')}`
    if (key > to) break
    out.push(key)
    month += 1
    if (month > 12) [year, month] = [year + 1, 1]
  }
  return out
}

export function mockMonthBalance(
  ctx: AnalyticsMockContext,
  filter: ReportFilter,
  carrySince: string | null,
): MonthBalance {
  const shown = monthsOf(filter, 'cashflow')
  const first = shown[0]
  const last = shown[shown.length - 1]
  const start = carrySince && carrySince < first ? carrySince : first
  const cash = cashflow(ctx, span(start, last), filter)
  const today = new Date().toISOString().slice(0, 7)
  let carried = 0
  const rows: MonthBalanceRow[] = []
  for (const row of cash.rows) {
    const spent = Number(row.fixed) + Number(row.daily)
    const left = Number(row.income) - spent
    const carries = carrySince !== null && row.month >= carrySince
    const carriedIn = carries ? carried : 0
    if (carries) carried += left
    if (row.month < first) continue
    rows.push({
      month: row.month,
      income: row.income,
      spent: money(spent),
      left: money(left),
      carried_in: money(carriedIn),
      total: money(left + carriedIn),
      current: row.month === today,
    })
  }
  const finished = rows.filter((row) => !row.current)
  const average = finished.length ? finished.reduce((sum, row) => sum + Number(row.spent), 0) / finished.length : 0
  const sum = (field: 'income' | 'spent' | 'left') => money(rows.reduce((total, row) => total + Number(row[field]), 0))
  const end = rows[rows.length - 1]
  const total: MonthBalanceRow = {
    month: end?.month ?? '',
    income: sum('income'),
    spent: sum('spent'),
    left: sum('left'),
    carried_in: end?.carried_in ?? '0',
    total: end?.total ?? '0',
    current: end?.current ?? false,
  }
  const { days, perDay, incomeMark, spentMark } = end
    ? balanceDays(end, ctx)
    : { days: [], perDay: null, incomeMark: null, spentMark: null }
  return {
    currency: cash.meta.currency,
    carry_since: carrySince,
    rows,
    average_spent: money(average),
    total,
    days,
    per_day: perDay === null ? null : money(perDay),
    income_mark: incomeMark,
    spent_mark: spentMark,
  }
}

/** The last month day by day from the same flows as its row, counted back from its total (as the backend does). */
function balanceDays(row: MonthBalanceRow, ctx: AnalyticsMockContext) {
  const net = new Map<string, number>()
  let incomeMark: MonthBalanceMark | null = null
  let spentMark: MonthBalanceMark | null = null
  for (const flow of flows(ctx)) {
    if (flow.month !== row.month) continue
    const income = flow.incoming && flow.group === 'income'
    const spent = !flow.incoming && flow.group !== 'service'
    if (!income && !spent) continue
    net.set(flow.date, (net.get(flow.date) ?? 0) + (income ? flow.value : -flow.value))
    const best = income ? incomeMark : spentMark
    if (!best || flow.value > Number(best.amount)) {
      const mark = { date: flow.date, amount: money(flow.value), category: flow.key }
      if (income) incomeMark = mark
      else spentMark = mark
    }
  }
  const allowance = mockCalendar(row.month, ctx).allowance
  const perDay = row.current && allowance !== null ? Number(allowance) : null
  const [year, month] = row.month.split('-').map(Number)
  const count = new Date(year, month, 0).getDate()
  const today = new Date().toISOString().slice(0, 10)
  const dates = Array.from({ length: count }, (_, index) => `${row.month}-${String(index + 1).padStart(2, '0')}`)
  const balances: (number | null)[] = dates.map(() => null)
  let after = 0
  for (let index = dates.length - 1; index >= 0; index--) {
    if (dates[index] > today) continue
    balances[index] = Number(row.total) - after
    after += net.get(dates[index]) ?? 0
  }
  const lastReal = balances.reduce<number>((last, value, index) => (value === null ? last : index), -1)
  const days: MonthBalanceDay[] = dates.map((date, index) => ({
    date,
    balance: balances[index] === null ? null : money(balances[index]),
    forecast:
      perDay !== null && lastReal >= 0 && index >= lastReal
        ? money(Number(row.total) - perDay * (index - lastReal))
        : null,
  }))
  return { days, perDay, incomeMark, spentMark }
}
