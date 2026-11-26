export const BUDGET_TARGETS = ['category', 'total', 'store'] as const
export type BudgetTargetKind = (typeof BUDGET_TARGETS)[number]

export const BUDGET_PERIODS: BudgetPeriod[] = ['month', 'week', 'day']

/** The share of a limit the near-limit notice fires at: `BUDGET_WARN_PERCENT` in core's notifications. */
export const BUDGET_WARN_PERCENT = 90

/** Category groups a limit may watch: what is spent, not what is earned or moved. */
export const BUDGET_CATEGORY_GROUPS = ['daily', 'fixed'] as const
