import type { Component } from 'vue'

import type { DashboardWidgetDef } from './dashboard-registry.types'

import { CashbackBonuses } from '@/widgets/cashback-bonuses'
import { DailyBudget } from '@/widgets/daily-budget'
import { DashboardAllocation } from '@/widgets/dashboard-allocation'
import { DashboardAssets } from '@/widgets/dashboard-assets'
import { DashboardBudgets } from '@/widgets/dashboard-budgets'
import { DashboardBybitSync } from '@/widgets/dashboard-bybit-sync'
import { DashboardFx } from '@/widgets/dashboard-fx'
import { DashboardGoals } from '@/widgets/dashboard-goals'
import { DashboardTotal } from '@/widgets/dashboard-total'
import { ExchangeBalances } from '@/widgets/exchange-balances'
import { FixedCosts } from '@/widgets/fixed-costs'
import { InterestTax } from '@/widgets/interest-tax'
import { UtilitiesDue } from '@/widgets/utilities-due'
import { UtilityStats } from '@/widgets/utility-stats'
import { MiniCalendar } from '@/widgets/mini-calendar'
import { MonthSummary } from '@/widgets/month-summary'
import { NetWorthCard } from '@/widgets/net-worth'
import { OnboardingChecklist } from '@/widgets/onboarding-checklist'
import { Spending7d } from '@/widgets/spending-7d'
import { UpcomingList } from '@/widgets/upcoming-list'

/** Columns of the dashboard grid. */
export const DASHBOARD_COLS = 12
/** Height of one grid row, px. */
export const DASHBOARD_ROW_HEIGHT = 40

/** A widget whose texts are `dashboard.widget.<id>` and `dashboard.widgetHint.<id>`. */
function widget(
  id: string,
  component: Component,
  [width, height, minW, minH]: [number, number, number, number],
  extra: Partial<DashboardWidgetDef> = {},
): DashboardWidgetDef {
  return {
    id,
    titleKey: `dashboard.widget.${id}`,
    descriptionKey: `dashboard.widgetHint.${id}`,
    component,
    w: width,
    h: height,
    minW,
    minH,
    ...extra,
  }
}

/** Every widget the dashboard can hold, in the catalog's order; sizes are `[w, h, minW, minH]` in grid cells. */
export const DASHBOARD_WIDGETS: DashboardWidgetDef[] = [
  widget('onboarding', OnboardingChecklist, [12, 6, 6, 5]),
  widget('dailyBudget', DailyBudget, [4, 5, 3, 4]),
  widget('monthSummary', MonthSummary, [4, 6, 3, 5]),
  widget('upcoming', UpcomingList, [4, 7, 3, 4], { props: { days: 14, limit: 6 } }),
  widget('total', DashboardTotal, [4, 4, 3, 3]),
  widget('netWorth', NetWorthCard, [4, 6, 3, 5], { link: { name: 'analytics-wealth' } }),
  widget('fx', DashboardFx, [4, 4, 2, 3]),
  widget('spending7d', Spending7d, [4, 6, 3, 4], { link: { name: 'transactions-calendar' } }),
  widget('miniCalendar', MiniCalendar, [4, 8, 3, 6], { link: { name: 'transactions-calendar' } }),
  widget('assets', DashboardAssets, [4, 7, 3, 4]),
  widget('goals', DashboardGoals, [6, 6, 3, 4], { link: { name: 'planning-goals' } }),
  widget('budgets', DashboardBudgets, [6, 6, 3, 4], { link: { name: 'planning-budgets' } }),
  widget('utilitiesDue', UtilitiesDue, [4, 5, 3, 4]),
  widget('utilityStats', UtilityStats, [8, 6, 4, 5]),
  widget('cashback', CashbackBonuses, [4, 6, 3, 4]),
  widget('interestTax', InterestTax, [4, 4, 3, 3]),
  widget('fixedCosts', FixedCosts, [6, 5, 3, 4]),
  widget('byChain', DashboardAllocation, [6, 8, 4, 6], { props: { by: 'chain' } }),
  widget('byStore', DashboardAllocation, [6, 8, 4, 6], { props: { by: 'store' } }),
  widget('exchangeBalances', ExchangeBalances, [8, 7, 6, 5], { requires: 'exchange' }),
  widget('bybitSync', DashboardBybitSync, [8, 8, 6, 5], { requires: 'exchange' }),
]

/**
 * Widgets a fresh vault starts with. Net worth once: `netWorth` (breakdown,
 * a year's line, a link to the report) rather than `total` next to it.
 */
export const DEFAULT_DASHBOARD = [
  'onboarding',
  'dailyBudget',
  'monthSummary',
  'upcoming',
  'netWorth',
  'assets',
  'spending7d',
  'utilitiesDue',
]
