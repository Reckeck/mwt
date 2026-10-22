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
import { DashboardHealth } from '@/widgets/dashboard-health'
import { DashboardPassive } from '@/widgets/dashboard-passive'
import { DashboardScenario } from '@/widgets/dashboard-scenario'
import { DashboardSpending } from '@/widgets/dashboard-spending'
import { DashboardTopup } from '@/widgets/dashboard-topup'
import { DashboardUtilities } from '@/widgets/dashboard-utilities'
import { ExchangeBalances } from '@/widgets/exchange-balances'
import { InterestTax } from '@/widgets/interest-tax'
import { MonthSummary } from '@/widgets/month-summary'
import { NetWorthCard } from '@/widgets/net-worth'
import { OnboardingChecklist } from '@/widgets/onboarding-checklist'
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

/**
 * Every widget the dashboard can hold, in the catalog's order; sizes are `[w, h, minW, minH]`
 * in grid cells (a row is 40px + 12px gap). `minH` is what the content needs, so «S» never
 * scrolls inside; the default leaves little empty space when there is no data yet.
 */
export const DASHBOARD_WIDGETS: DashboardWidgetDef[] = [
  widget('onboarding', OnboardingChecklist, [12, 5, 6, 4]),
  widget('dailyBudget', DailyBudget, [4, 5, 3, 4]),
  widget('monthSummary', MonthSummary, [4, 6, 3, 5]),
  widget('upcoming', UpcomingList, [4, 6, 3, 4], { props: { days: 7, limit: 6 }, home: true }),
  widget('netWorth', NetWorthCard, [4, 6, 3, 4], { link: { name: 'money-wealth' }, periodic: true }),
  widget('fx', DashboardFx, [4, 5, 2, 4], { home: true }),
  widget('spending', DashboardSpending, [4, 6, 3, 4], { link: { name: 'transactions-calendar' }, periodic: true }),
  widget('assets', DashboardAssets, [4, 7, 3, 4], { home: true }),
  widget('health', DashboardHealth, [4, 5, 3, 3], { link: { name: 'money-health' } }),
  widget('passiveIncome', DashboardPassive, [4, 5, 3, 3], { link: { name: 'money-health' } }),
  widget('topupAdvisor', DashboardTopup, [4, 6, 3, 4], { link: { name: 'money-investments' } }),
  widget('scenario', DashboardScenario, [4, 5, 3, 3], { link: { name: 'money-calculator' }, scenario: true }),
  widget('goals', DashboardGoals, [6, 5, 3, 4], { link: { name: 'money-plan' } }),
  widget('budgets', DashboardBudgets, [6, 5, 3, 4], { link: { name: 'money-plan' } }),
  widget('utilities', DashboardUtilities, [6, 6, 3, 5], { home: true }),
  widget('allocation', DashboardAllocation, [6, 9, 4, 7]),
  widget('cashback', CashbackBonuses, [4, 6, 3, 4], { link: { name: 'transactions-bonuses' } }),
  widget('interestTax', InterestTax, [4, 4, 3, 3]),
  widget('exchangeBalances', ExchangeBalances, [8, 5, 6, 4], { requires: 'exchange' }),
  widget('bybitSync', DashboardBybitSync, [8, 5, 6, 4], { requires: 'exchange' }),
]

/**
 * Widgets a fresh vault starts with: none. Home shows its three figures, the things
 * to do and what changed; widgets are the user's choice (Home 5).
 */
export const DEFAULT_DASHBOARD: string[] = []
