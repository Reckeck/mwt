/** Commands: automations, budgets, goals and their simulation. */

import { txMeta } from '@/shared/lib/transactions'
import { currentMonth, dailyBudget, goalProgress, money, monthSummary, monthTotals } from './budget-calc'
import { dropWorkflow, listed, mainOf, syncWorkflow } from './automation-workflow'
import { deleteGoal, restoreGoal } from './goal-delete'
import {
  automations,
  budgets,
  draftOf,
  fail,
  goals,
  iso,
  moveOffDayOff,
  now,
  remove,
  replace,
  runs,
  uuid,
} from './state'
import type { PlanningMockContext } from './state'

/** Planning commands: Automations, budgets, goals and their simulation. `undefined` when `cmd` is not one. */
export function planAutomations(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
  settings: Settings,
): unknown {
  const body = args?.input as Record<string, unknown> | undefined
  const rate = Number(settings.savings_rate)
  switch (cmd) {
    case 'list_automations':
      return automations.map(listed)
    case 'create_automation': {
      const input = body as AutomationInput
      const automation: Automation = {
        id: uuid(),
        name: input.name.trim() || fail('name the automation'),
        active: input.active,
        preset: input.preset ?? null,
        schedule: input.schedule,
        starts_at: input.starts_at,
        template: draftOf(ctx.transactionFrom(input.template, input.starts_at)),
        last_planned_at: null,
        created_at: iso(now()),
        amount_mode: input.amount_mode ?? 'fixed',
        workday_shift: input.workday_shift ?? 'none',
        property_id: input.template.common.property_id ?? null,
        estimate: null,
      }
      automations.push(automation)
      syncWorkflow(automation)
      return listed(automation)
    }
    case 'update_automation': {
      const input = body as UpdateAutomationInput
      const current = automations.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      return updated(
        replace(automations, input.id, {
          ...current,
          name: input.automation.name,
          active: input.automation.active,
          schedule: input.automation.schedule,
          starts_at: input.automation.starts_at,
          template: draftOf(ctx.transactionFrom(input.automation.template, input.automation.starts_at)),
          amount_mode: input.automation.amount_mode ?? 'fixed',
          workday_shift: input.automation.workday_shift ?? 'none',
          property_id: input.automation.template.common.property_id ?? null,
        }),
      )
    }
    case 'delete_automation':
      remove(automations, String(args?.id))
      dropWorkflow(String(args?.id))
      return {}
    case 'run_automation': {
      const automation = automations.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      const tx = structuredClone(automation.template) as Transaction
      const estimated = automation.amount_mode === 'varies'
      const main = mainOf(tx)
      const estimate = estimated ? listed(automation).estimate : null
      if (estimate && main) {
        const inner = 'topup' in tx ? tx.topup : 'payment' in tx ? tx.payment : 'transfer' in tx ? tx.transfer : null
        if (inner) inner.amount = estimate
      }
      const runId = uuid()
      txMeta(tx).id = uuid()
      txMeta(tx).timestamp = iso(now())
      txMeta(tx).origin = {
        workflow: { workflow_id: automation.id, run_id: runId, planned_at: iso(now()), estimated },
      }
      ctx.transactions.unshift(tx)
      runs.push({
        id: runId,
        automation_id: automation.id,
        planned_at: iso(now()),
        processed_at: iso(now()),
        outcome: { created: { tx_id: txMeta(tx).id } },
        estimated,
        amount: estimate ?? main?.amount ?? null,
        symbol: main?.symbol ?? '',
      })
      return tx
    }
    case 'list_automation_runs':
      return runs
        .filter((run) => !args?.automationId || run.automation_id === args.automationId)
        .sort((left, right) => right.planned_at.localeCompare(left.planned_at))
    case 'retry_automation_run':
      return fail('mock: 0 USD of 800 USD', 'INSUFFICIENT_FUNDS')
    case 'skip_automation_run': {
      const run = runs.find((entry) => entry.id === args?.runId) ?? fail('not found', 'MISSING')
      run.outcome = 'skipped'
      return run
    }
    case 'list_budgets':
      return budgets
    case 'create_budget': {
      const input = body as BudgetInput
      const budget: Budget = { id: uuid(), ...input, warn: input.warn ?? true }
      budgets.push(budget)
      return budget
    }
    case 'update_budget': {
      const input = body as UpdateBudgetInput
      return replace(budgets, input.id, { ...input.budget, warn: input.budget.warn ?? true })
    }
    case 'delete_budget':
      remove(budgets, String(args?.id))
      return {}
    case 'get_budget_status': {
      const totals = monthTotals(ctx, currentMonth())
      return budgets
        .filter((budget) => budget.active)
        .map((budget): BudgetStatus => {
          const spent = totals.fixed + totals.daily
          const limit = Number(budget.limit)
          return {
            budget,
            spent: money(spent),
            remaining: money(limit - spent),
            used: money(limit ? spent / limit : 0),
            period_start: `${currentMonth()}-01T00:00:00Z`,
            period_end: `${currentMonth()}-28T00:00:00Z`,
          }
        })
    }
    case 'get_daily_budget':
      return dailyBudget(ctx, settings.daily_budget_mode, rate)
    case 'get_month_summary':
      return monthSummary(ctx, String(args?.month ?? currentMonth()), rate)
    case 'list_goals':
      return [...goals].sort((left, right) => right.priority - left.priority)
    case 'create_goal': {
      const goal: Goal = { id: uuid(), ...(body as GoalInput), created_at: iso(now()) }
      goals.push(goal)
      return goal
    }
    case 'update_goal': {
      const input = body as UpdateGoalInput
      const current = goals.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      return replace(goals, input.id, { ...current, ...input.goal })
    }
    case 'delete_goal':
      return deleteGoal(String(args?.id)) ?? fail('not found', 'MISSING')
    case 'restore_goal':
      return restoreGoal(args?.deleted as GoalDeleted)
    case 'get_goal_progress':
      return goalProgress(ctx)
    case 'get_goal_calendar': {
      const year = Number(args?.year)
      const months: GoalCalendarMonth[] = Array.from({ length: 12 }, (_unused, index) => {
        const month = `${year}-${String(index + 1).padStart(2, '0')}`
        const need = goals.filter((goal) => goal.active && goal.deadline).length ? 100 : 0
        return { month, need: money(need), saved: '0', shortfall: money(need), done: need === 0 }
      })
      return { year, currency: ctx.budgetCurrency, months } satisfies GoalCalendar
    }
    case 'simulate_automations': {
      // A thin stand-in for the sandbox run: monthly automations, every run created.
      const input = body as SimulateAutomationsInput
      const until = Date.now() + input.days * 86_400_000
      const runs: SimulatedRun[] = []
      for (const automation of automations.filter((item) => !input.automation_id || item.id === input.automation_id)) {
        if (!('monthly' in automation.schedule)) continue
        const at = new Date()
        at.setDate(automation.schedule.monthly.day)
        at.setHours(9, 0, 0, 0)
        if (at.getTime() < Date.now()) at.setMonth(at.getMonth() + 1)
        for (; at.getTime() <= until; at.setMonth(at.getMonth() + 1)) {
          runs.push({
            automation_id: automation.id,
            automation_name: automation.name,
            planned_at: moveOffDayOff(at, automation.workday_shift).toISOString(),
            outcome: { created: { tx_id: uuid() } },
            transaction: automation.template as Transaction,
          })
        }
      }
      runs.sort((left, right) => left.planned_at.localeCompare(right.planned_at))
      return {
        from: iso(now()),
        until: new Date(until).toISOString(),
        runs,
        balances: [],
        created: runs.length,
        rejected: 0,
      } satisfies AutomationSimulation
    }
    default:
      return undefined
  }
}

/** An automation saved in the form: its workflow follows, the list shows its estimate. */
function updated(automation: Automation): Automation {
  syncWorkflow(automation)
  return listed(automation)
}
