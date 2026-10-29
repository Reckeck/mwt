/** The salary and the rent of the demo vault booked by automations: the row's «авто» has something to tell (Paths 2 · 10). */

import { txMeta } from '@/shared/lib/transactions'
import { syncWorkflow } from './automation-workflow'
import { automations, draftOf, iso, now, runs, utilities, uuid } from './state'

/** `YYYY-MM` of the month `offset` months from the one the moment falls in (the core's `for_month_key`). */
export function forMonthKey(timestamp: string, offset: number): string {
  const at = new Date(timestamp)
  const month = new Date(at.getFullYear(), at.getMonth() + offset, 1)
  return `${month.getFullYear()}-${String(month.getMonth() + 1).padStart(2, '0')}`
}

/** Make the transactions with this comment an automation's runs; the latest one's day is the schedule's. */
function seedAutomation(
  transactions: Transaction[],
  comment: string,
  name: string,
  preset: AutomationPreset | null = null,
  forMonthOffset?: number,
) {
  const booked = transactions
    .filter((tx) => ('topup' in tx || 'payment' in tx) && txMeta(tx).comment === comment)
    .sort((left, right) => txMeta(left).timestamp.localeCompare(txMeta(right).timestamp))
  const latest = booked.at(-1)
  if (!latest) return null
  const automation: Automation = {
    id: uuid(),
    name,
    active: true,
    preset,
    schedule: { monthly: { day: new Date(txMeta(latest).timestamp).getDate() } },
    starts_at: txMeta(booked[0]).timestamp,
    template: draftOf(structuredClone(latest)),
    last_planned_at: txMeta(latest).timestamp,
    created_at: iso(now()),
    amount_mode: 'topup' in latest ? 'varies' : 'fixed',
    workday_shift: 'back',
    property_id: null,
    estimate: null,
    time: null,
    asks: false,
    ...(forMonthOffset === undefined ? {} : { for_month_offset: forMonthOffset }),
  }
  automations.push(automation)
  syncWorkflow(automation)
  for (const tx of booked) {
    const meta = txMeta(tx)
    const runId = uuid()
    meta.origin = {
      workflow: { workflow_id: automation.id, run_id: runId, planned_at: meta.timestamp, estimated: false },
    }
    // The details object is shared between the demo's operations: a copy, not a write.
    if ('topup' in tx && forMonthOffset !== undefined) {
      tx.topup.details = { ...tx.topup.details, for_month: forMonthKey(meta.timestamp, forMonthOffset) }
    }
    const leg = 'topup' in tx ? tx.topup : 'payment' in tx ? tx.payment : null
    runs.push({
      id: runId,
      automation_id: automation.id,
      planned_at: meta.timestamp,
      processed_at: meta.timestamp,
      outcome: { created: { tx_id: meta.id } },
      estimated: false,
      amount: leg?.amount ?? null,
      symbol: leg?.asset.symbol ?? '',
    })
  }
  return automation
}

export function seedSalaryAutomation(transactions: Transaction[]) {
  // The salary comes on the 5th for the month before: its rows carry "за <месяц>" (Russian "for <month>").
  seedAutomation(transactions, 'Зарплата', 'Зарплата', 'salary', -1)
  // The flat's rent item names the automation that pays it (Wallets-2 · 10, Wallets-3 · 17).
  const rent = seedAutomation(transactions, 'Аренда квартиры', 'Аренда')
  for (const item of utilities) if (rent && 'housing' in item.kind) item.kind.housing.automation_id = rent.id
}
