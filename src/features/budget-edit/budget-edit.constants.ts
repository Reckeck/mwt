export const BUDGET_TARGETS = ['category', 'total', 'store'] as const
export type BudgetTargetKind = (typeof BUDGET_TARGETS)[number]

export const BUDGET_PERIODS: BudgetPeriod[] = ['month', 'week', 'day']

/** The share of a limit the near-limit notice fires at: `BUDGET_WARN_PERCENT` in core's notifications. */
export const BUDGET_WARN_PERCENT = 90
