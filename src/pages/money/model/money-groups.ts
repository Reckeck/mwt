/**
 * The «Money» section: four tabs one goes to often (spending, where it went, the plan, shared events)
 * and five behind «More» (capital, investments, the month's balance, the checkup, the calculator).
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
