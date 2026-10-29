import type { PeriodPreset } from '@/shared/lib/period'

/**
 * The «Money» section: four tabs one goes to often (spending, where it went, the
 * plan, shared events) and five behind «More» (capital, investments, the month's balance, the
 * checkup, the calculator).
 */
export type MoneyGroup =
  | 'spending'
  | 'where'
  | 'plan'
  | 'together'
  | 'wealth'
  | 'investments'
  | 'month'
  | 'health'
  | 'calculator'

/** «Where it went» shows one of three reports. */
export type FlowView = 'months' | 'year' | 'categories'

export const FLOW_VIEWS: FlowView[] = ['months', 'year', 'categories']

export const FLOW_VIEW_KIND: Record<FlowView, ReportKind> = {
  months: 'cashflow',
  year: 'year',
  categories: 'categories',
}

/** Route name of each tab. */
export const GROUP_ROUTE: Record<MoneyGroup, string> = {
  spending: 'money',
  where: 'money-where',
  plan: 'money-plan',
  together: 'money-together',
  wealth: 'money-wealth',
  investments: 'money-investments',
  month: 'money-month',
  health: 'money-health',
  calculator: 'money-calculator',
}

/** The strip; the rest wait behind «More», in this order. */
export const MAIN_GROUPS: MoneyGroup[] = ['spending', 'where', 'plan', 'together']
export const MORE_GROUPS: MoneyGroup[] = ['wealth', 'investments', 'month', 'health', 'calculator']

/** What the export offers on a tab with a fixed report (the flow view is added at runtime). */
export const GROUP_EXPORTS: Partial<Record<MoneyGroup, ReportKind[]>> = {
  spending: ['categories', 'cashflow'],
  wealth: ['networth', 'stores'],
  investments: ['portfolio', 'pnl', 'losses'],
}

/** Tabs where «in today's money» means something; elsewhere the figures stay nominal. */
export const REAL_TERMS_GROUPS: MoneyGroup[] = ['where', 'wealth']

/**
 * The period a tab opens with while the address has none: the month for spending
 * and its balance, the last twelve months for the rest.
 */
export const GROUP_DEFAULT: Partial<Record<MoneyGroup, PeriodPreset>> = {
  spending: 'thisMonth',
  where: 'thisMonth',
  plan: 'thisMonth',
  month: 'thisMonth',
}

/** Tabs where the wallets do not apply: the balance counts every wallet. */
export const ALL_WALLETS_GROUPS: MoneyGroup[] = ['month']

/** Tabs that judge «now» or plan ahead and take no filter. */
export const UNFILTERED_GROUPS: MoneyGroup[] = ['plan', 'together', 'health', 'calculator']

export function flowViewOf(raw: unknown): FlowView {
  return FLOW_VIEWS.includes(raw as FlowView) ? (raw as FlowView) : 'months'
}
