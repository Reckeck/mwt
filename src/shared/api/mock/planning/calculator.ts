/** Investment calculator commands and saved scenarios (plan A). */

import { txLegs } from '@/shared/lib/transactions'
import { money } from './budget-calc'
import { compare, plannedBalance, simulate, solve } from './invest-calc'
import { fail, goals, iso, now, uuid } from './state'
import type { PlanningMockContext } from './state'

export const scenarios: CalcScenario[] = []

const SCENARIO_GOAL_PRIORITY = 3
const PER_MONTH: Record<Freq, number> = { monthly: 1, quarterly: 1 / 3, yearly: 1 / 12 }

const findScenario = (id: string) => scenarios.find((entry) => entry.id === id) ?? fail(`scenario ${id}`, 'MISSING')

function save(draft: CalcScenarioDraft): CalcScenario {
  if (!draft.name.trim()) fail('scenario name is empty')
  const stamp = iso(now())
  const current = draft.id ? findScenario(draft.id) : null
  const scenario: CalcScenario = {
    id: current?.id ?? uuid(),
    name: draft.name.trim(),
    input: draft.input,
    source: draft.source,
    prefilled_at: draft.prefilled_at,
    created_at: current?.created_at ?? stamp,
    updated_at: stamp,
  }
  if (current) scenarios.splice(scenarios.indexOf(current), 1, scenario)
  else scenarios.push(scenario)
  return scenario
}

function remove(id: string): CalcScenarioDeleted {
  const scenario = findScenario(id)
  scenarios.splice(scenarios.indexOf(scenario), 1)
  const linked = goals.filter((goal) => goal.scenario_id === id)
  for (const goal of linked) goal.scenario_id = null
  return { scenario, goal_ids: linked.map((goal) => goal.id) }
}

function restore(deleted: CalcScenarioDeleted): CalcScenario {
  if (scenarios.some((entry) => entry.id === deleted.scenario.id)) fail('scenario already exists', 'CONFLICT')
  scenarios.push(deleted.scenario)
  for (const goal of goals.filter((entry) => deleted.goal_ids.includes(entry.id)))
    goal.scenario_id = deleted.scenario.id
  return deleted.scenario
}

function prefill(source: ScenarioSource, ctx: PlanningMockContext): ScenarioPrefill {
  const ids = new Set(source.scope.store_ids)
  const initial = ctx.transactions
    .flatMap((tx) => txLegs(tx))
    .filter((leg) => leg.role === 'principal' && ids.has(leg.store_id))
    .reduce((sum, leg) => sum + Number(leg.amount) * ctx.priceOf(leg.asset.symbol), 0)
  const income = 1500
  return {
    initial: money(initial),
    rate_pct: null,
    capitalization: null,
    pays_out: false,
    rate_store_id: null,
    monthly_contribution: money((income * Number(source.income_pct)) / 100),
    income_avg: money(income),
    income_months_used: source.income_months,
    income_manual: false,
    prefilled_at: iso(now()),
  }
}

function goalFrom(id: string): GoalInput {
  const scenario = findScenario(id)
  const today = now()
  today.setHours(0, 0, 0, 0)
  const deadline = new Date(today)
  deadline.setMonth(deadline.getMonth() + scenario.input.term_months)
  const first = [...scenario.input.stages].sort((left, right) => left.from_month - right.from_month)[0]
  return {
    name: scenario.name,
    active: true,
    target_amount: simulate(scenario.input).final_nominal,
    currency: scenario.input.currency.toLowerCase(),
    starts_at: iso(today),
    deadline: iso(deadline),
    priority: SCENARIO_GOAL_PRIORITY,
    monthly_commitment: first ? money(Number(first.amount) * PER_MONTH[first.freq]) : null,
    scope: scenario.source?.scope ?? { store_ids: [], include_children: false },
    scenario_id: scenario.id,
    expected_return: Number(scenario.input.rate_pct) > 0 ? scenario.input.rate_pct : null,
  }
}

/** «По плану к сегодня» of a goal that follows a scenario. */
export function plannedNow(goal: Goal): string | null {
  const scenario = scenarios.find((entry) => entry.id === goal.scenario_id)
  if (!scenario) return null
  const start = new Date(goal.starts_at)
  const today = now()
  const elapsed = (today.getFullYear() - start.getFullYear()) * 12 + today.getMonth() - start.getMonth()
  return money(plannedBalance(scenario.input, elapsed))
}

export function planCalculator(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
): unknown {
  switch (cmd) {
    case 'calc_invest_simulate':
      return simulate(args?.input as InvestInput)
    case 'calc_invest_solve':
      return solve(args?.input as InvestInput, Number(args?.target), args?.unknown as SolveUnknown)
    case 'calc_invest_compare':
      return compare(args?.input as InvestInput)
    case 'calc_scenarios_list':
      return [...scenarios].sort((left, right) => right.updated_at.localeCompare(left.updated_at))
    case 'calc_scenario_save':
      return save(args?.draft as CalcScenarioDraft)
    case 'calc_scenario_delete':
      return remove(String(args?.id))
    case 'calc_scenario_restore':
      return restore(args?.deleted as CalcScenarioDeleted)
    case 'calc_scenario_prefill':
      return prefill(args?.source as ScenarioSource, ctx)
    case 'goal_from_scenario':
      return goalFrom(String(args?.id))
    default:
      return undefined
  }
}
