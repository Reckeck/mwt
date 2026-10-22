/** A limit's pace (plan H3): how much of the period went by, where spending ends it, and the risk. */
export interface BudgetPace {
  status: BudgetStatus
  /** Share of the period that went by, 0..1. */
  elapsed: number
  /** Spending at today's pace by the period's end. */
  forecast: number
  /** Forecast over the limit; `> 1` is an overspend ahead. */
  risk: number
}

const DAY_MS = 86_400_000

const dayOf = (iso: string) => new Date(`${iso.slice(0, 10)}T12:00`).getTime()

export function budgetPace(status: BudgetStatus, now = new Date()): BudgetPace {
  const start = dayOf(status.period_start)
  const end = dayOf(status.period_end)
  const span = Math.max(1, Math.round((end - start) / DAY_MS) + 1)
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 12).getTime()
  const passed = Math.min(span, Math.max(1, Math.round((today - start) / DAY_MS) + 1))
  const elapsed = passed / span
  const spent = Number(status.spent)
  const forecast = spent / elapsed
  const limit = Number(status.budget.limit)
  return { status, elapsed, forecast, risk: limit > 0 ? forecast / limit : 0 }
}

/** The riskiest limits first: an overspend ahead matters more than one already spent at a slow pace. */
export const byRisk = (statuses: BudgetStatus[]) =>
  statuses.map((status) => budgetPace(status)).sort((left, right) => right.risk - left.risk)
