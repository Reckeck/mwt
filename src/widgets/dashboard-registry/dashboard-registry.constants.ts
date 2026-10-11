import type { Component } from 'vue'

import { CashbackBonuses } from '@/widgets/cashback-bonuses'
import { DailyBudget } from '@/widgets/daily-budget'
import { DashboardAllocation } from '@/widgets/dashboard-allocation'
import { DashboardAssets } from '@/widgets/dashboard-assets'
import { DashboardBudgets } from '@/widgets/dashboard-budgets'
import { DashboardFx } from '@/widgets/dashboard-fx'
import { DashboardGoals } from '@/widgets/dashboard-goals'
import { DashboardTotal } from '@/widgets/dashboard-total'
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

export interface DashboardWidgetDef {
  id: string
  titleKey: string
  descriptionKey: string
  component: Component
  /** Props the component needs on the dashboard. */
  props?: Record<string, unknown>
  /** Default size in grid cells. */
  w: number
  h: number
  minW: number
  minH: number
  /** Only offered when the vault has an exchange. */
  requires?: 'exchange'
}

/** Every widget the dashboard can hold, in the catalog's order. */
export const DASHBOARD_WIDGETS: DashboardWidgetDef[] = [
  {
    id: 'onboarding',
    titleKey: 'dashboard.widget.onboarding',
    descriptionKey: 'dashboard.widgetHint.onboarding',
    component: OnboardingChecklist,
    w: 12,
    h: 4,
    minW: 6,
    minH: 3,
  },
  {
    id: 'dailyBudget',
    titleKey: 'dashboard.widget.dailyBudget',
    descriptionKey: 'dashboard.widgetHint.dailyBudget',
    component: DailyBudget,
    w: 4,
    h: 5,
    minW: 3,
    minH: 4,
  },
  {
    id: 'monthSummary',
    titleKey: 'dashboard.widget.monthSummary',
    descriptionKey: 'dashboard.widgetHint.monthSummary',
    component: MonthSummary,
    w: 4,
    h: 5,
    minW: 3,
    minH: 4,
  },
  {
    id: 'upcoming',
    titleKey: 'dashboard.widget.upcoming',
    descriptionKey: 'dashboard.widgetHint.upcoming',
    component: UpcomingList,
    props: { days: 14, limit: 6 },
    w: 4,
    h: 7,
    minW: 3,
    minH: 4,
  },
  {
    id: 'total',
    titleKey: 'dashboard.widget.total',
    descriptionKey: 'dashboard.widgetHint.total',
    component: DashboardTotal,
    w: 4,
    h: 4,
    minW: 3,
    minH: 3,
  },
  {
    id: 'netWorth',
    titleKey: 'dashboard.widget.netWorth',
    descriptionKey: 'dashboard.widgetHint.netWorth',
    component: NetWorthCard,
    w: 4,
    h: 6,
    minW: 3,
    minH: 4,
  },
  {
    id: 'fx',
    titleKey: 'dashboard.widget.fx',
    descriptionKey: 'dashboard.widgetHint.fx',
    component: DashboardFx,
    w: 4,
    h: 4,
    minW: 2,
    minH: 3,
  },
  {
    id: 'spending7d',
    titleKey: 'dashboard.widget.spending7d',
    descriptionKey: 'dashboard.widgetHint.spending7d',
    component: Spending7d,
    w: 4,
    h: 6,
    minW: 3,
    minH: 4,
  },
  {
    id: 'miniCalendar',
    titleKey: 'dashboard.widget.miniCalendar',
    descriptionKey: 'dashboard.widgetHint.miniCalendar',
    component: MiniCalendar,
    w: 4,
    h: 8,
    minW: 3,
    minH: 6,
  },
  {
    id: 'assets',
    titleKey: 'dashboard.widget.assets',
    descriptionKey: 'dashboard.widgetHint.assets',
    component: DashboardAssets,
    w: 4,
    h: 7,
    minW: 3,
    minH: 4,
  },
  {
    id: 'goals',
    titleKey: 'dashboard.widget.goals',
    descriptionKey: 'dashboard.widgetHint.goals',
    component: DashboardGoals,
    w: 6,
    h: 6,
    minW: 3,
    minH: 4,
  },
  {
    id: 'budgets',
    titleKey: 'dashboard.widget.budgets',
    descriptionKey: 'dashboard.widgetHint.budgets',
    component: DashboardBudgets,
    w: 6,
    h: 6,
    minW: 3,
    minH: 4,
  },
  {
    id: 'utilitiesDue',
    titleKey: 'dashboard.widget.utilitiesDue',
    descriptionKey: 'dashboard.widgetHint.utilitiesDue',
    component: UtilitiesDue,
    w: 4,
    h: 5,
    minW: 3,
    minH: 4,
  },
  {
    id: 'utilityStats',
    titleKey: 'dashboard.widget.utilityStats',
    descriptionKey: 'dashboard.widgetHint.utilityStats',
    component: UtilityStats,
    w: 8,
    h: 6,
    minW: 4,
    minH: 5,
  },
  {
    id: 'cashback',
    titleKey: 'dashboard.widget.cashback',
    descriptionKey: 'dashboard.widgetHint.cashback',
    component: CashbackBonuses,
    w: 4,
    h: 6,
    minW: 3,
    minH: 4,
  },
  {
    id: 'interestTax',
    titleKey: 'dashboard.widget.interestTax',
    descriptionKey: 'dashboard.widgetHint.interestTax',
    component: InterestTax,
    w: 4,
    h: 4,
    minW: 3,
    minH: 3,
  },
  {
    id: 'fixedCosts',
    titleKey: 'dashboard.widget.fixedCosts',
    descriptionKey: 'dashboard.widgetHint.fixedCosts',
    component: FixedCosts,
    w: 6,
    h: 5,
    minW: 3,
    minH: 3,
  },
  {
    id: 'byChain',
    titleKey: 'dashboard.widget.byChain',
    descriptionKey: 'dashboard.widgetHint.byChain',
    component: DashboardAllocation,
    props: { by: 'chain' },
    w: 6,
    h: 8,
    minW: 4,
    minH: 6,
  },
  {
    id: 'byStore',
    titleKey: 'dashboard.widget.byStore',
    descriptionKey: 'dashboard.widgetHint.byStore',
    component: DashboardAllocation,
    props: { by: 'store' },
    w: 6,
    h: 8,
    minW: 4,
    minH: 6,
  },
]

/** Widgets a fresh vault starts with. */
export const DEFAULT_DASHBOARD = [
  'onboarding',
  'dailyBudget',
  'monthSummary',
  'upcoming',
  'total',
  'netWorth',
  'spending7d',
  'utilitiesDue',
]

export function widgetDef(id: string): DashboardWidgetDef | undefined {
  return DASHBOARD_WIDGETS.find((def) => def.id === id)
}

/**
 * Place widgets left to right, top to bottom in a 12-column grid. The grid
 * compacts upwards afterwards, so this only needs to avoid overlaps.
 */
export function packWidgets(ids: string[]): DashboardWidgetCfg[] {
  const out: DashboardWidgetCfg[] = []
  let column = 0
  let row = 0
  let rowHeight = 0
  for (const id of ids) {
    const def = widgetDef(id)
    if (!def) continue
    if (column + def.w > DASHBOARD_COLS) {
      column = 0
      row += rowHeight
      rowHeight = 0
    }
    out.push({ id, col: column, row, width: def.w, height: def.h, pinned: false, store_id: null })
    column += def.w
    rowHeight = Math.max(rowHeight, def.h)
  }
  return out
}
