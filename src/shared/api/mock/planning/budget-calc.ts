/** Month totals, the daily budget, goal progress and the agenda. */

import { txLegs, txMeta } from '@/shared/lib/transactions'
import { estimateOf } from './automation-workflow'
import { plannedNow } from './calculator'
import { automations, goals, isRejected, iso, now, plannedDates, reminders, runs } from './state'
import type { PlanningMockContext } from './state'

function groupOf(tx: Transaction, ctx: PlanningMockContext): CategoryGroup | null {
  for (const key of txMeta(tx).categories) {
    const category = ctx.categories.find((entry) => entry.key === key)
    if (category) return category.group
  }
  return null
}

/** Money value of a transaction in USD by its principal legs on budget stores. */
function flowValue(tx: Transaction, ctx: PlanningMockContext, incoming: boolean): number {
  const inBudget = new Set(
    ctx.stores.filter((store) => store.purpose === 'everyday' && store.kind !== 'liability').map((store) => store.id),
  )
  return txLegs(tx)
    .filter((leg) => leg.role === 'principal' && inBudget.has(leg.store_id))
    .filter((leg) => Number(leg.amount) > 0 === incoming)
    .reduce(
      (sum, leg) => sum + Math.abs(Number(leg.amount)) * (Number(leg.price_usd) || ctx.priceOf(leg.asset.symbol)),
      0,
    )
}

export function monthTotals(ctx: PlanningMockContext, month: string) {
  const totals = { income: 0, fixed: 0, daily: 0, perDay: new Map<string, number>() }
  for (const tx of ctx.transactions) {
    const stamp = txMeta(tx).timestamp
    if (!stamp.startsWith(month)) continue
    const group = groupOf(tx, ctx)
    if ('topup' in tx && group === 'income') totals.income += flowValue(tx, ctx, true)
    if ('payment' in tx && group === 'fixed') totals.fixed += flowValue(tx, ctx, false)
    if ('payment' in tx && group !== 'fixed' && group !== 'service' && group !== 'income') {
      const spent = flowValue(tx, ctx, false)
      totals.daily += spent
      const day = stamp.slice(0, 10)
      totals.perDay.set(day, (totals.perDay.get(day) ?? 0) + spent)
    }
  }
  return totals
}

export const currentMonth = () => iso(now()).slice(0, 7)
export const money = (value: number) => String(Math.round(value * 100) / 100)

export function monthSummary(ctx: PlanningMockContext, month: string, savingsRate: number): MonthSummary {
  const totals = monthTotals(ctx, month)
  const fact = totals.income - totals.fixed - totals.daily
  const plan = totals.income * savingsRate
  return {
    month,
    currency: ctx.budgetCurrency,
    income: money(totals.income),
    fixed: money(totals.fixed),
    daily: money(totals.daily),
    savings_plan: money(plan),
    savings_fact: money(fact),
    deviation: money(fact - plan),
    saved_pct: totals.income ? money((fact / totals.income) * 100) : '0',
    savings_moved: '0',
    budget_in: '0',
    budget_out: '0',
    not_executed: runs.filter((run) => isRejected(run.outcome) && run.planned_at.startsWith(month)).length,
    complete: true,
  }
}

export function dailyBudget(ctx: PlanningMockContext, mode: DailyBudgetMode, savingsRate: number): DailyBudget {
  const today = iso(now()).slice(0, 10)
  const totals = monthTotals(ctx, currentMonth())
  const days = new Date(now().getFullYear(), now().getMonth() + 1, 0).getDate()
  const perDay = (totals.income - totals.fixed - totals.income * savingsRate) / days
  let carry = 0
  for (let day = 1; day < now().getDate(); day++) {
    const key = `${currentMonth()}-${String(day).padStart(2, '0')}`
    carry += perDay - (totals.perDay.get(key) ?? 0)
  }
  const spent = totals.perDay.get(today) ?? 0
  const available = perDay + carry
  return {
    mode,
    currency: ctx.budgetCurrency,
    date: today,
    per_day: money(perDay),
    carry: money(carry),
    available: money(available),
    spent_today: money(spent),
    left_today: money(available - spent),
    income: money(totals.income),
    fixed: money(totals.fixed),
    savings_plan: money(totals.income * savingsRate),
    free: '0',
    days,
    next_income: null,
    manual_limit: null,
  }
}

export function goalProgress(ctx: PlanningMockContext): GoalProgress[] {
  return goals
    .filter((goal) => goal.active)
    .map((goal) => {
      const ids = new Set(goal.scope.store_ids)
      const saved = ctx.transactions
        .flatMap((tx) => txLegs(tx))
        .filter((leg) => leg.role === 'principal' && ids.has(leg.store_id))
        .reduce((sum, leg) => sum + Number(leg.amount) * ctx.priceOf(leg.asset.symbol), 0)
      const target = Number(goal.target_amount)
      const deadline = goal.deadline ? new Date(goal.deadline) : null
      const monthsLeft = deadline
        ? Math.max(1, (deadline.getFullYear() - now().getFullYear()) * 12 + deadline.getMonth() - now().getMonth() + 1)
        : null
      const need = monthsLeft ? Math.max(0, target - saved) / monthsLeft : null
      const issues: RealismIssue[] = []
      if (need !== null && goal.monthly_commitment && need > Number(goal.monthly_commitment)) {
        issues.push({ over_commitment: { need: money(need), commitment: goal.monthly_commitment } })
      }
      return {
        goal,
        saved: money(saved),
        contributed: money(saved),
        fx_gain: '0',
        pct: money(target ? (saved / target) * 100 : 0),
        months_total: monthsLeft,
        months_left: monthsLeft,
        need_per_month: need === null ? null : money(need),
        realistic: issues.length === 0,
        issues,
        forecast: null,
        accumulating: false,
        planned_now: plannedNow(goal),
      }
    })
}

export function upcoming(days: number): UpcomingItem[] {
  const until = new Date(now().getTime() + days * 86_400_000)
  const items: UpcomingItem[] = []
  for (const automation of automations.filter((entry) => entry.active)) {
    const estimate = automation.amount_mode === 'varies' ? estimateOf(automation.id) : null
    const dates = plannedDates(automation.schedule, new Date(automation.starts_at), until, automation.workday_shift)
    for (const date of dates) {
      if (date <= now()) continue
      const tx = automation.template as Transaction
      const leg = txLegs(tx).find((entry) => entry.role === 'principal')
      items.push({
        kind: 'automation',
        source_id: automation.id,
        at: iso(date),
        date: iso(date).slice(0, 10),
        title: automation.name,
        store_id: leg?.store_id ?? null,
        amount: estimate ?? leg?.amount ?? null,
        asset: leg?.asset ?? null,
        shortfall: null,
        estimated: automation.amount_mode === 'varies',
      })
    }
  }
  // Booked approximately: waits for the real amount.
  for (const run of runs.filter(
    (entry) => entry.estimated && typeof entry.outcome === 'object' && 'created' in entry.outcome,
  )) {
    const outcome = run.outcome as { created: { tx_id: string } }
    items.push({
      kind: 'unconfirmed_run',
      source_id: outcome.created.tx_id,
      at: run.planned_at,
      date: run.planned_at.slice(0, 10),
      title: automations.find((entry) => entry.id === run.automation_id)?.name ?? '',
      store_id: null,
      amount: run.amount,
      asset: assetOf(run.automation_id),
      shortfall: null,
      estimated: true,
    })
  }
  for (const reminder of reminders.filter((entry) => !entry.done && new Date(entry.due_at) <= until)) {
    items.push({
      kind: 'reminder',
      source_id: reminder.id,
      at: reminder.due_at,
      date: reminder.due_at.slice(0, 10),
      title: reminder.text,
      store_id: null,
      amount: null,
      asset: null,
      shortfall: null,
      estimated: false,
    })
  }
  for (const run of runs.filter((entry) => isRejected(entry.outcome))) {
    items.push({
      kind: 'rejected_run',
      source_id: run.id,
      at: run.planned_at,
      date: run.planned_at.slice(0, 10),
      title: automations.find((entry) => entry.id === run.automation_id)?.name ?? '',
      store_id: null,
      amount: null,
      asset: null,
      shortfall: null,
      estimated: false,
    })
  }
  return items.sort((left, right) => left.at.localeCompare(right.at))
}

/** The asset an automation books, for a line without its own. */
function assetOf(automationId: string): AssetRef | null {
  const template = automations.find((entry) => entry.id === automationId)?.template
  return template ? (txLegs(template as Transaction)[0]?.asset ?? null) : null
}
