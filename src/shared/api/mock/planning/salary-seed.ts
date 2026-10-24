/** The salary and the rent of the demo vault booked by automations: the row's «авто» has something to tell (Paths 2 · 10). */

import { txMeta } from '@/shared/lib/transactions'
import { syncWorkflow } from './automation-workflow'
import { automations, draftOf, iso, now, runs, utilities, uuid } from './state'

/** Make the transactions with this comment an automation's runs; the latest one's day is the schedule's. */
function seedAutomation(transactions: Transaction[], comment: string, name: string) {
  const booked = transactions
    .filter((tx) => ('topup' in tx || 'payment' in tx) && txMeta(tx).comment === comment)
    .sort((left, right) => txMeta(left).timestamp.localeCompare(txMeta(right).timestamp))
  const latest = booked.at(-1)
  if (!latest) return null
  const automation: Automation = {
    id: uuid(),
    name,
    active: true,
    preset: null,
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
  }
  automations.push(automation)
  syncWorkflow(automation)
  for (const tx of booked) {
    const meta = txMeta(tx)
    const runId = uuid()
    meta.origin = {
      workflow: { workflow_id: automation.id, run_id: runId, planned_at: meta.timestamp, estimated: false },
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
  seedAutomation(transactions, 'Зарплата', 'Зарплата')
  // The flat's rent item names the automation that pays it (Wallets-2 · 10, Wallets-3 · 17).
  const rent = seedAutomation(transactions, 'Аренда квартиры', 'Аренда')
  for (const item of utilities) if (rent && 'housing' in item.kind) item.kind.housing.automation_id = rent.id
}
