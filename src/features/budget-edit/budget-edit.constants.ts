export const BUDGET_TARGETS = ['total', 'category', 'store'] as const
export type BudgetTargetKind = (typeof BUDGET_TARGETS)[number]

export const BUDGET_PERIODS: BudgetPeriod[] = ['day', 'week', 'month']
