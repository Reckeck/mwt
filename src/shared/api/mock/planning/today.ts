/** The dashboard «Сегодня» strip and the scenario widget (plan H4–H5). */

import { mockNetWorthTrend } from '../analytics/dashboard-tiles'
import { mockSummary } from '../market'
import { dailyBudget, money, upcoming } from './budget-calc'
import { scenarios } from './calculator'
import { capitalHealth } from './health'
import { plannedBalance, simulate } from './invest-calc'
import { fail, goals, now } from './state'
import type { PlanningMockContext } from './state'

const TODAY_TODOS = 3
const TODAY_HORIZON_DAYS = 14

function todos(): TodayTodo[] {
  const items: TodayTodo[] = upcoming(TODAY_HORIZON_DAYS)
    .filter((item) => item.shortfall !== null || item.kind === 'rejected_run' || item.kind === 'unconfirmed_run')
    .map((item) => ({
      kind: item.shortfall !== null ? 'shortfall' : (item.kind as TodayTodoKind),
      date: item.date,
      title: item.title,
      source_id: item.source_id,
      amount: item.shortfall,
    }))
  return items.sort((left, right) => (left.date ?? '').localeCompare(right.date ?? '')).slice(0, TODAY_TODOS)
}

function dashboardToday(ctx: PlanningMockContext, settings: Settings, period: DashPeriod): DashboardToday {
  const trend = mockNetWorthTrend(period, Number(mockSummary.total_base), mockSummary.base_currency)
  const budget = dailyBudget(ctx, settings.daily_budget_mode, Number(settings.savings_rate))
  return {
    period,
    currency: trend.currency,
    net_worth: trend.change,
    health: capitalHealth(ctx, settings).color,
    left_today: budget.left_today,
    budget_currency: budget.currency,
    todos: todos(),
  }
}

function scenarioStatus(id: string): ScenarioStatus {
  const scenario = scenarios.find((entry) => entry.id === id) ?? fail(`scenario ${id}`, 'MISSING')
  const [year, month] = (scenario.input.start ?? '').split('-').map(Number)
  const today = now()
  const since = year ? (today.getFullYear() - year) * 12 + today.getMonth() + 1 - month : 0
  const elapsed = Math.min(Math.max(since, 0), scenario.input.term_months)
  const goal = goals.find((entry) => entry.active && entry.scenario_id === id)
  const planned = plannedBalance(scenario.input, elapsed)
  return {
    id,
    name: scenario.name,
    currency: scenario.input.currency,
    term_months: scenario.input.term_months,
    elapsed_months: elapsed,
    final_nominal: simulate(scenario.input).final_nominal,
    planned_now: money(planned),
    fact: scenario.source || goal ? money(planned * 0.93) : null,
    goal_id: goal?.id ?? null,
  }
}

export function planToday(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
  settings: Settings,
): unknown {
  switch (cmd) {
    case 'get_dashboard_today':
      return dashboardToday(ctx, settings, (args?.period as DashPeriod | undefined) ?? settings.dashboard_period)
    case 'get_scenario_status':
      return scenarioStatus(String(args?.id))
    default:
      return undefined
  }
}
