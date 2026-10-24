/** «Мои» (Auto-1 · 1): everything that fires on its own, as `list_my_automations` gives it. */

import { txLegs } from '@/shared/lib/transactions'
import { workflows } from '../workflows/state'
import { alerts, automations, iso, now, plannedDates, reminders } from './state'

const LOOKAHEAD_DAYS = 400

function item(kind: MineKind, id: string, name: string, active: boolean): MineItem {
  return {
    kind,
    id,
    name,
    active,
    next_at: null,
    amount: null,
    asset: null,
    from_store: null,
    to_store: null,
    asks: false,
    notifies: false,
    condition: null,
    currency: null,
    description: '',
  }
}

/** The money of an operation: where it leaves, where it comes, how much. */
function withOperation(row: MineItem, tx: Transaction): MineItem {
  const legs = txLegs(tx).filter((leg) => leg.role === 'principal')
  const out = legs.find((leg) => Number(leg.amount) < 0)
  const into = legs.find((leg) => Number(leg.amount) > 0)
  const leg = into ?? out
  return {
    ...row,
    from_store: out?.store_id ?? null,
    to_store: into?.store_id ?? null,
    amount: leg?.amount ?? null,
    asset: leg?.asset ?? null,
  }
}

/** A scheme's next run: the mock reads only a daily «HH:MM», tomorrow at that time. */
function nextScheme(graph: WorkflowGraph): string | null {
  const schedule = graph.nodes.find((node) => node.kind === 'trigger.schedule' && !node.disabled)
  if (!schedule) return null
  const [hours, minutes] = String((schedule.params as { time?: string }).time ?? '09:00')
    .split(':')
    .map(Number)
  const next = now()
  next.setDate(next.getDate() + 1)
  next.setHours(hours || 0, minutes || 0, 0, 0)
  return iso(next)
}

export function myAutomations(): MineItem[] {
  const until = new Date(now().getTime() + LOOKAHEAD_DAYS * 86_400_000)
  const rows: MineItem[] = []
  for (const automation of automations) {
    const next = plannedDates(
      automation.schedule,
      new Date(automation.starts_at),
      until,
      automation.workday_shift,
    ).find((date) => date > now())
    const row = withOperation(
      item('repeat', automation.id, automation.name, automation.active),
      automation.template as Transaction,
    )
    rows.push({ ...row, next_at: next ? iso(next) : null })
  }
  for (const workflow of workflows.filter((entry) => !entry.automation)) {
    const graph = JSON.parse(workflow.graph) as WorkflowGraph
    const live = graph.nodes.filter((node) => !node.disabled)
    const operates = live.some((node) =>
      ['action.transaction', 'action.distribute', 'action.transfer'].includes(node.kind),
    )
    rows.push({
      ...item('scheme', workflow.id, workflow.name, workflow.active),
      description: workflow.description,
      next_at: nextScheme(graph),
      asks: live.some((node) => node.kind === 'action.confirm'),
      notifies: !operates && live.some((node) => node.kind === 'action.notify' || node.kind === 'action.reminder'),
    })
  }
  for (const reminder of reminders.filter((entry) => !entry.done)) {
    rows.push({
      ...item('reminder', reminder.id, reminder.text, !reminder.muted),
      next_at: reminder.snoozed_until ?? reminder.due_at,
      notifies: true,
    })
  }
  for (const alert of alerts) {
    rows.push({
      ...item('alert', alert.id, alert.asset.symbol.toUpperCase(), alert.active && !alert.triggered_at),
      amount: alert.price,
      asset: alert.asset,
      condition: alert.condition,
      currency: alert.currency,
      notifies: true,
    })
  }
  const when = (row: MineItem) => (row.next_at ? Date.parse(row.next_at) : Number.POSITIVE_INFINITY)
  return rows.sort(
    (left, right) => when(left) - when(right) || left.name.toLowerCase().localeCompare(right.name.toLowerCase()),
  )
}

export function setMyAutomationActive(input: SetMyAutomationActiveInput) {
  if (input.kind === 'reminder') {
    const reminder = reminders.find((entry) => entry.id === input.id)
    if (reminder) reminder.muted = !input.active
  } else if (input.kind === 'alert') {
    const alert = alerts.find((entry) => entry.id === input.id)
    if (alert) Object.assign(alert, { active: input.active }, input.active ? { triggered_at: null } : {})
  } else {
    const automation = automations.find((entry) => entry.id === input.id)
    if (automation) automation.active = input.active
    const workflow = workflows.find((entry) => entry.id === input.id)
    if (workflow) workflow.active = input.active
  }
  return null
}
