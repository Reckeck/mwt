/**
 * Browser-mock half of planning: automations, budgets, goals, reminders,
 * alerts and the agenda for `pnpm dev`. The numbers are rough stand-ins for
 * clicking through the UI; the real rules (funds check, carry-over, shares
 * of shared stores) live only in the backend.
 */

import { txLegs, txMeta } from '@/shared/lib/transactions'

/** What the planning mock needs from the ledger mock. */
export interface PlanningMockContext {
  transactions: Transaction[]
  categories: Category[]
  stores: Store[]
  priceOf: (symbol: string) => number
  transactionFrom: (input: TransactionInput, ts: string) => Transaction
  budgetCurrency: string
}

const automations: Automation[] = []
const runs: AutomationRun[] = []
const budgets: Budget[] = []
const goals: Goal[] = []
const reminders: Reminder[] = []
const alerts: PriceAlert[] = []
const utilities: UtilityItem[] = []
const readings: MeterReading[] = []

/** Bills by month of a metered item: consumption × tariff. */
function utilityTrend(item: UtilityItem): UtilityTrend {
  const sortKey = (entry: MeterReading) => `${entry.period}-${String(entry.day ?? 99).padStart(2, '0')}`
  const own = readings
    .filter((entry) => entry.item_id === item.id)
    .sort((left, right) => sortKey(left).localeCompare(sortKey(right)))
  const tariff = 'metered' in item.kind ? Number(item.kind.metered.tariff) : 0
  const paid = paymentsOf(item)
  // Like `monthly_readings` in Rust: a month runs from the last reading
  // before it, the first month of a meter from its own first reading.
  const periods = [...new Set(own.map((entry) => entry.period))]
  const months: UtilityMonth[] =
    'metered' in item.kind
      ? periods.map((period, index) => {
          const group = own.filter((entry) => entry.period === period)
          const last = group[group.length - 1]
          const before = index ? own.filter((entry) => entry.period === periods[index - 1]).at(-1) : undefined
          const start = before ?? (group.length > 1 ? group[0] : undefined)
          const consumption = start ? Number(last.value) - Number(start.value) : null
          return {
            period,
            reading: last.value,
            consumption: consumption === null ? null : String(consumption),
            amount: money((consumption ?? 0) * tariff),
            reading_id: last.id,
            payments: paid.get(period) ?? [],
            readings: group,
          }
        })
      : [...paid.entries()]
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([period, payments]) => ({
            period,
            reading: null,
            consumption: null,
            amount: money(payments.reduce((sum, tx) => sum + ('payment' in tx ? Number(tx.payment.amount) : 0), 0)),
            reading_id: null,
            payments,
            readings: [],
          }))
  const billed = months.map((month) => Number(month.amount)).filter((value) => value > 0)
  const average = billed.length ? billed.reduce((sum, value) => sum + value, 0) / billed.length : null
  const fixed =
    'static' in item.kind ? item.kind.static.amount : 'housing' in item.kind ? item.kind.housing.amount : '0'
  return {
    item_id: item.id,
    currency: item.currency,
    months,
    average: average === null ? null : money(average),
    delta_pct: null,
    forecast: average === null ? null : money(average),
    anomaly: false,
    monthly: billed.length ? money(billed[billed.length - 1]) : fixed,
  }
}

/** Two bills of a demo flat with half a year of readings. */
export function seedUtilityDemo(propertyId: string) {
  if (utilities.length) return
  const base = { active: true, currency: 'rub', property_id: propertyId, store_id: null, category: 'utilities' }
  const power: UtilityItem = {
    id: uuid(),
    name: 'Электричество',
    kind: { metered: { unit: 'кВт·ч', tariff: '6.5' } },
    ...base,
  }
  const net: UtilityItem = { id: uuid(), name: 'Интернет', kind: { static: { amount: '700' } }, ...base }
  utilities.push(power, net)
  const now = new Date()
  let value = 12000
  for (let back = 6; back >= 1; back -= 1) {
    const month = new Date(now.getFullYear(), now.getMonth() - back, 1)
    const period = `${month.getFullYear()}-${String(month.getMonth() + 1).padStart(2, '0')}`
    value += 180 + back * 15
    readings.push({
      id: uuid(),
      item_id: power.id,
      value: String(value),
      period,
      recorded_at: month.toISOString(),
      day: null,
    })
  }
}

/** Payments linked to a bill, by `YYYY-MM`. */
function paymentsOf(item: UtilityItem): Map<string, Transaction[]> {
  const out = new Map<string, Transaction[]>()
  for (const tx of mockContext?.transactions ?? []) {
    if (!('payment' in tx) || tx.payment.meta.utility_item_id !== item.id) continue
    const period = tx.payment.meta.timestamp.slice(0, 7)
    out.set(period, [...(out.get(period) ?? []), tx])
  }
  return out
}
let mockContext: PlanningMockContext | null = null

const uuid = () => crypto.randomUUID()
const isRejected = (outcome: AutomationOutcome) => typeof outcome === 'object' && 'rejected' in outcome
const now = () => new Date()
const iso = (date: Date) => date.toISOString()
const fail = (message: string, code = 'INVALID_INPUT') => {
  throw { code, message }
}

function draftOf(tx: Transaction): TransactionDraft {
  if ('topup' in tx || 'payment' in tx || 'transfer' in tx || 'convert' in tx) return tx as TransactionDraft
  return fail('an automation repeats a top-up, payment, transfer or exchange')
}

/** Monthly dates (clamped to the month), weekly, every N days — from `start` up to `until`. */
function plannedDates(schedule: Schedule, start: Date, until: Date): Date[] {
  const out: Date[] = []
  const at = (year: number, month: number, day: number) => new Date(year, month, day, 9)
  if ('monthly' in schedule) {
    let cursor = new Date(start.getFullYear(), start.getMonth(), 1)
    while (cursor <= until) {
      const last = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 0).getDate()
      const date = at(cursor.getFullYear(), cursor.getMonth(), Math.min(schedule.monthly.day, last))
      if (date >= start && date <= until) out.push(date)
      cursor = new Date(cursor.getFullYear(), cursor.getMonth() + 1, 1)
    }
  } else {
    const step = 'weekly' in schedule ? 7 : Math.max(1, schedule.every_n_days.days)
    const cursor = at(start.getFullYear(), start.getMonth(), start.getDate())
    if ('weekly' in schedule) {
      const weekday = (cursor.getDay() + 6) % 7
      cursor.setDate(cursor.getDate() + ((schedule.weekly.weekday - weekday + 7) % 7))
    }
    while (cursor <= until) {
      if (cursor >= start) out.push(new Date(cursor))
      cursor.setDate(cursor.getDate() + step)
    }
  }
  return out
}

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

function monthTotals(ctx: PlanningMockContext, month: string) {
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

const currentMonth = () => iso(now()).slice(0, 7)
const money = (value: number) => String(Math.round(value * 100) / 100)

function monthSummary(ctx: PlanningMockContext, month: string, savingsRate: number): MonthSummary {
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

function dailyBudget(ctx: PlanningMockContext, mode: DailyBudgetMode, savingsRate: number): DailyBudget {
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

function goalProgress(ctx: PlanningMockContext): GoalProgress[] {
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
      }
    })
}

function upcoming(days: number): UpcomingItem[] {
  const until = new Date(now().getTime() + days * 86_400_000)
  const items: UpcomingItem[] = []
  for (const automation of automations.filter((entry) => entry.active)) {
    for (const date of plannedDates(automation.schedule, new Date(automation.starts_at), until)) {
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
        amount: leg?.amount ?? null,
        asset: leg?.asset ?? null,
        shortfall: null,
      })
    }
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
    })
  }
  return items.sort((left, right) => left.at.localeCompare(right.at))
}

function replace<T extends { id: string }>(list: T[], id: string, next: Omit<T, 'id'>): T {
  const index = list.findIndex((entry) => entry.id === id)
  if (index < 0) fail('not found', 'MISSING')
  list[index] = { ...list[index], ...next, id }
  return list[index]
}

function remove<T extends { id: string }>(list: T[], id: string) {
  const index = list.findIndex((entry) => entry.id === id)
  if (index < 0) fail('not found', 'MISSING')
  list.splice(index, 1)
}

/**
 * Answer a planning command, or `undefined` when `cmd` is not one.
 * `settings` supplies the savings rate and the daily budget mode.
 */
export function mockPlanning(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
  settings: Settings,
): unknown {
  const body = args?.input as Record<string, unknown> | undefined
  const rate = Number(settings.savings_rate)
  mockContext = ctx
  switch (cmd) {
    case 'list_automations':
      return automations
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
      }
      automations.push(automation)
      return automation
    }
    case 'update_automation': {
      const input = body as UpdateAutomationInput
      const current = automations.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      return replace(automations, input.id, {
        ...current,
        name: input.automation.name,
        active: input.automation.active,
        schedule: input.automation.schedule,
        starts_at: input.automation.starts_at,
        template: draftOf(ctx.transactionFrom(input.automation.template, input.automation.starts_at)),
      })
    }
    case 'delete_automation':
      remove(automations, String(args?.id))
      return {}
    case 'run_automation': {
      const automation = automations.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      const tx = structuredClone(automation.template) as Transaction
      txMeta(tx).id = uuid()
      txMeta(tx).timestamp = iso(now())
      ctx.transactions.unshift(tx)
      runs.push({
        id: uuid(),
        automation_id: automation.id,
        planned_at: iso(now()),
        processed_at: iso(now()),
        outcome: { created: { tx_id: txMeta(tx).id } },
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
      const budget: Budget = { id: uuid(), ...(body as BudgetInput) }
      budgets.push(budget)
      return budget
    }
    case 'update_budget': {
      const input = body as UpdateBudgetInput
      return replace(budgets, input.id, input.budget)
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
      remove(goals, String(args?.id))
      return {}
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
            planned_at: at.toISOString(),
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
    case 'list_reminders':
      return [...reminders].sort((left, right) => left.due_at.localeCompare(right.due_at))
    case 'create_reminder': {
      const input = body as ReminderInput
      const reminder: Reminder = {
        id: uuid(),
        text: input.text,
        due_at: input.due_at,
        done: false,
        repeat: input.repeat ?? null,
        source: null,
        muted: false,
        user_edited: false,
        snoozed_until: null,
        source_key: '',
      }
      reminders.push(reminder)
      return reminder
    }
    case 'update_reminder': {
      const input = body as UpdateReminderInput
      const current = reminders.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      return replace(reminders, input.id, {
        ...current,
        ...input.reminder,
        repeat: input.reminder.repeat ?? null,
        snoozed_until: null,
      })
    }
    case 'delete_reminder': {
      // An automatic one is muted, not removed: its source would make it again.
      const reminder = reminders.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      if (reminder.source) Object.assign(reminder, { repeat: null, done: true, snoozed_until: null, muted: true })
      else remove(reminders, reminder.id)
      return {}
    }
    case 'set_reminder_done': {
      const reminder = reminders.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      reminder.snoozed_until = null
      if (args?.done && reminder.repeat) {
        // Like `next_occurrence`: counted from the due date, past now.
        const anchor = new Date(reminder.due_at)
        const { every, unit } = reminder.repeat
        for (let count = 1; count < 10_000; count += 1) {
          const next = new Date(anchor)
          if (unit === 'day') next.setUTCDate(anchor.getUTCDate() + every * count)
          if (unit === 'week') next.setUTCDate(anchor.getUTCDate() + every * count * 7)
          if (unit === 'month') next.setUTCMonth(anchor.getUTCMonth() + every * count)
          if (unit === 'year') next.setUTCFullYear(anchor.getUTCFullYear() + every * count)
          if (next.getTime() > Date.now()) {
            reminder.due_at = next.toISOString()
            break
          }
        }
      } else reminder.done = Boolean(args?.done)
      return reminder
    }
    case 'finish_reminder_series': {
      const reminder = reminders.find((entry) => entry.id === args?.id) ?? fail('not found', 'MISSING')
      Object.assign(reminder, { repeat: null, done: true, snoozed_until: null })
      return reminder
    }
    case 'snooze_reminder': {
      const input = body as SnoozeReminderInput
      const reminder = reminders.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      reminder.snoozed_until = input.until
      return reminder
    }
    case 'list_price_alerts':
      return alerts
    case 'create_price_alert': {
      const input = body as PriceAlertInput
      const symbol = input.asset.symbol.toUpperCase()
      const alert: PriceAlert = {
        id: uuid(),
        asset: { ...input.asset, symbol, quote: symbol },
        condition: input.condition,
        price: input.price,
        currency: input.currency,
        active: input.active,
        triggered_at: null,
        ...(input.store_id ? { store_id: input.store_id } : {}),
      }
      alerts.push(alert)
      return alert
    }
    case 'update_price_alert': {
      const input = body as UpdatePriceAlertInput
      const symbol = input.alert.asset.symbol.toUpperCase()
      return replace(alerts, input.id, {
        ...input.alert,
        asset: { ...input.alert.asset, symbol, quote: symbol },
        triggered_at: null,
      })
    }
    case 'delete_price_alert':
      remove(alerts, String(args?.id))
      return {}
    case 'list_utility_items':
      return utilities
    case 'create_utility_item': {
      const item: UtilityItem = { id: uuid(), ...(body as UtilityInput) }
      utilities.push(item)
      return item
    }
    case 'update_utility_item': {
      const input = body as UpdateUtilityInput
      return replace(utilities, input.id, input.item)
    }
    case 'delete_utility_item':
      remove(utilities, String(args?.id))
      return {}
    case 'add_meter_reading': {
      const input = body as MeterReadingInput
      const reading: MeterReading = { id: uuid(), ...input, day: input.day ?? null, recorded_at: iso(now()) }
      const index = readings.findIndex((entry) => entry.item_id === input.item_id && entry.period === input.period)
      if (index >= 0) readings.splice(index, 1)
      readings.push(reading)
      return reading
    }
    case 'delete_meter_reading':
      remove(readings, String(args?.id))
      return {}
    case 'update_meter_reading': {
      const input = body as UpdateMeterReadingInput
      const reading = readings.find((entry) => entry.id === input.id) ?? fail('not found', 'MISSING')
      Object.assign(reading, { value: input.value, period: input.period, day: input.day ?? null })
      return reading
    }
    case 'get_utility_bill': {
      const propertyId = String(args?.propertyId ?? '')
      const period = String(args?.period ?? iso(now()).slice(0, 7))
      const property = ctx.stores.find((store) => store.id === propertyId)
      const payDay = property?.property?.utilities_pay_day ?? null
      const lines: UtilityBillLine[] = utilities
        .filter((item) => item.active && item.property_id === propertyId)
        .map((item) => {
          const own = readings
            .filter((entry) => entry.item_id === item.id)
            .sort((left, right) => left.period.localeCompare(right.period))
          const previous = own.filter((entry) => entry.period < period).at(-1)?.value ?? null
          const current = own.find((entry) => entry.period === period)?.value ?? null
          const tariff = 'metered' in item.kind ? Number(item.kind.metered.tariff) : 0
          const fixed =
            'static' in item.kind ? item.kind.static.amount : 'housing' in item.kind ? item.kind.housing.amount : null
          const consumption = current !== null && previous !== null ? Number(current) - Number(previous) : null
          const paid = (paymentsOf(item).get(period) ?? []).reduce(
            (sum, tx) => sum + ('payment' in tx ? Number(tx.payment.amount) : 0),
            0,
          )
          return {
            item,
            previous_reading: previous,
            reading: current,
            consumption: consumption === null ? null : String(consumption),
            amount: 'metered' in item.kind ? (consumption === null ? null : money(consumption * tariff)) : fixed,
            paid: money(paid),
            includes_deposit: false,
          }
        })
      const day = String(Math.min(payDay ?? now().getDate(), 28)).padStart(2, '0')
      return {
        property_id: propertyId,
        period,
        pay_day: payDay,
        due_at: `${period}-${day}T12:00:00Z`,
        lines,
      } satisfies UtilityBill
    }
    case 'pay_utility_bill': {
      const input = body as PayUtilityBillInput
      const saved: Transaction[] = []
      for (const line of input.lines) {
        const item = utilities.find((entry) => entry.id === line.item_id) ?? fail('not found', 'MISSING')
        if (line.reading) {
          remove(
            readings,
            readings.find((entry) => entry.item_id === item.id && entry.period === input.period)?.id ?? '',
          )
          readings.push({
            id: uuid(),
            item_id: item.id,
            value: line.reading,
            period: input.period,
            recorded_at: iso(now()),
            day: line.reading_day ?? null,
          })
        }
        const code = item.currency.toUpperCase()
        const ts = input.timestamp || `${input.period}-10T12:00:00Z`
        const tx = ctx.transactionFrom(
          {
            type: 'payment',
            common: {
              timestamp: ts,
              comment: item.name,
              categories: [item.category],
              flow_id: null,
              contact_id: null,
              price_usd: null,
              fx_override: null,
              price: null,
              price_currency: null,
            },
            store_id: item.store_id || input.store_id,
            asset: { symbol: code, name: code, coingecko_id: null, chain: 'fiat', contract: null },
            amount: line.amount,
            fee: null,
            to: null,
            details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null },
          } as TransactionInput,
          ts,
        )
        if ('payment' in tx) tx.payment.meta.utility_item_id = item.id
        ctx.transactions.push(tx)
        saved.push(tx)
      }
      return saved
    }
    case 'get_property_summary': {
      const storeId = String(args?.storeId ?? '')
      const own = utilities.filter((item) => item.active && item.property_id === storeId)
      const monthly = own.reduce((sum, item) => sum + Number(utilityTrend(item).monthly), 0)
      const summary: PropertySummary = {
        store_id: storeId,
        base_currency: 'usd',
        value_base: '0',
        debt_base: '0',
        equity_base: '0',
        utilities_month_base: String(monthly),
        loan_month_base: '0',
        monthly_cost_base: String(monthly),
        utility_count: own.length,
      }
      return summary
    }
    case 'get_meter_trend': {
      const item = utilities.find((entry) => entry.id === args?.itemId) ?? fail('not found', 'MISSING')
      return utilityTrend(item)
    }
    case 'get_utility_payment_draft': {
      const item = utilities.find((entry) => entry.id === args?.itemId) ?? fail('not found', 'MISSING')
      const trend = utilityTrend(item)
      const code = item.currency.toUpperCase()
      const tx = ctx.transactionFrom(
        {
          type: 'payment',
          common: {
            timestamp: null,
            comment: item.name,
            categories: [item.category],
            flow_id: null,
            contact_id: null,
            price_usd: null,
            fx_override: null,
            price: null,
            price_currency: null,
          },
          store_id: item.store_id ?? '',
          asset: { symbol: code, name: code, coingecko_id: null, chain: 'fiat', contract: null },
          amount: trend.monthly,
          fee: null,
          to: null,
          details: { cash_breakdown: null, unit_price: null, network: null, tx_hash: null },
        } as TransactionInput,
        iso(now()),
      )
      return {
        item_id: item.id,
        period: String(args?.period ?? iso(now()).slice(0, 7)),
        transaction: tx,
        consumption: null,
        includes_deposit: false,
      } satisfies UtilityPaymentDraft
    }
    case 'get_upcoming':
      return upcoming(Number(args?.days ?? 30))
    default:
      return undefined
  }
}
