/**
 * Automations as workflows in the mock, the way the vault keeps them: each
 * one has a workflow of the same id built from its form; an amount that
 * differs is booked at the average of the latest corrected ones.
 */

import { txLegs, txMeta } from '@/shared/lib/transactions'
import { workflows } from '../workflows/state'
import { automations, fail, iso, now, runs } from './state'
import type { PlanningMockContext } from './state'

/** How many latest corrected amounts make the estimate. */
const ESTIMATE_OF = 3

/** The amount a top-up, a payment or a transfer moves; `null` for anything else. */
export function mainOf(tx: Transaction): { amount: string; symbol: string } | null {
  const inner = 'topup' in tx ? tx.topup : 'payment' in tx ? tx.payment : 'transfer' in tx ? tx.transfer : null
  return inner ? { amount: inner.amount, symbol: inner.asset.symbol } : null
}

/** The average of the latest corrected amounts, or `null` with none yet. */
export function estimateOf(automationId: string): string | null {
  const amounts = runs
    .filter((run) => run.automation_id === automationId && !run.estimated && run.amount)
    .sort((left, right) => right.planned_at.localeCompare(left.planned_at))
    .slice(0, ESTIMATE_OF)
    .map((run) => Number(run.amount))
  if (!amounts.length) return null
  return (amounts.reduce((sum, value) => sum + value, 0) / amounts.length).toFixed(2)
}

/** The automation as listed: with its current estimate. */
export function listed(automation: Automation): Automation {
  return { ...automation, estimate: automation.amount_mode === 'varies' ? estimateOf(automation.id) : null }
}

function graphOf(automation: Automation): string {
  const schedule = automation.schedule
  const every = 'monthly' in schedule ? 'month' : 'weekly' in schedule ? 'week' : 'day'
  const trigger = {
    id: 't',
    kind: 'trigger.schedule',
    params: {
      every,
      day: 'monthly' in schedule ? schedule.monthly.day : 1,
      weekday: 'weekly' in schedule ? schedule.weekly.weekday : 4,
      interval: 'every_n_days' in schedule ? String(schedule.every_n_days.days) : '1',
      time: '09:00',
      workday: automation.workday_shift,
    },
    position: { x: 0, y: 0 },
  }
  // The readable fields the editor shows; the operation itself comes from the template.
  const tx = automation.template as Transaction
  const kind = 'topup' in tx ? 'topup' : 'payment' in tx ? 'payment' : 'transfer' in tx ? 'transfer' : 'convert'
  const leg = txLegs(tx)[0]
  const action = {
    id: 'tx',
    kind: 'action.transaction',
    params: {
      tx_kind: kind,
      account: leg ? { store: leg.store_id } : null,
      to: 'transfer' in tx ? tx.transfer.to : null,
      asset: leg?.asset ?? null,
      amount: mainOf(tx)?.amount ?? '0',
      category: txMeta(tx).categories[0] ?? null,
      comment: txMeta(tx).comment,
      template: JSON.stringify(automation.template),
      estimated: automation.amount_mode === 'varies',
      on_shortfall: 'reject',
    },
    position: { x: 560, y: 0 },
  }
  const edge = (from: string, fromPort: string, to: string, toPort: string) => ({
    id: `${from}.${fromPort}>${to}.${toPort}`,
    from,
    from_port: fromPort,
    to,
    to_port: toPort,
  })
  return JSON.stringify({
    version: 3,
    nodes: [trigger, action],
    edges: [edge('t', 'out', 'tx', 'in'), edge('t', 'at', 'tx', 'date')],
    comments: [],
  })
}

/** Keep the automation's workflow in step with its form. */
export function syncWorkflow(automation: Automation) {
  const spec: AutomationSpec = {
    preset: automation.preset,
    schedule: automation.schedule,
    starts_at: automation.starts_at,
    template: automation.template,
    amount_mode: automation.amount_mode,
    workday_shift: automation.workday_shift,
    property_id: automation.property_id,
    time: automation.time,
    asks: automation.asks,
  }
  const workflow: Workflow = {
    id: automation.id,
    name: automation.name,
    description: '',
    active: automation.active,
    graph: graphOf(automation),
    created_at: automation.created_at,
    updated_at: iso(now()),
    react_to_workflows: false,
    max_amount: null,
    versions: [],
    tests: [],
    automation: spec,
  }
  const index = workflows.findIndex((entry) => entry.id === automation.id)
  if (index < 0) workflows.push(workflow)
  else workflows[index] = workflow
}

export function dropWorkflow(id: string) {
  const index = workflows.findIndex((entry) => entry.id === id)
  if (index >= 0) workflows.splice(index, 1)
}

/** `confirm_estimated_amount` and `detach_automation`; `undefined` for anything else. */
export function planAutomationAmounts(
  cmd: string,
  args: Record<string, unknown> | undefined,
  ctx: PlanningMockContext,
): unknown {
  switch (cmd) {
    case 'confirm_estimated_amount': {
      const input = args?.input as ConfirmEstimatedInput
      const run = runs.find(
        (entry) =>
          typeof entry.outcome === 'object' &&
          'created' in entry.outcome &&
          entry.outcome.created.tx_id === input.tx_id,
      )
      if (!run?.estimated) return fail('this operation was not booked with an approximate amount')
      if (!(Number(input.amount) > 0)) return fail('the amount must be above zero')
      const tx = ctx.transactions.find((entry) => txMeta(entry).id === input.tx_id) ?? fail('not found', 'MISSING')
      const inner = 'topup' in tx ? tx.topup : 'payment' in tx ? tx.payment : 'transfer' in tx ? tx.transfer : null
      if (inner) inner.amount = input.amount
      const origin = txMeta(tx).origin
      if (typeof origin === 'object' && 'workflow' in origin) origin.workflow.estimated = false
      run.estimated = false
      run.amount = input.amount
      return tx
    }
    case 'detach_automation': {
      const id = String(args?.id)
      const index = automations.findIndex((entry) => entry.id === id)
      if (index < 0) return fail(`automation ${id}`, 'MISSING')
      automations.splice(index, 1)
      const workflow = workflows.find((entry) => entry.id === id) ?? fail(`workflow ${id}`, 'MISSING')
      workflow.automation = undefined
      return workflow
    }
    default:
      return undefined
  }
}
