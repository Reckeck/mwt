/** The salary of the demo vault booked by an automation: the row's «авто» has something to tell (Paths 2 · 10). */

import { txMeta } from '@/shared/lib/transactions'
import { syncWorkflow } from './automation-workflow'
import { automations, draftOf, iso, now, runs, uuid } from './state'

/** Make the salaries an automation's runs; the latest one's day is the schedule's. */
export function seedSalaryAutomation(transactions: Transaction[]) {
  const salaries = transactions
    .filter((tx) => 'topup' in tx && txMeta(tx).comment === 'Зарплата')
    .sort((left, right) => txMeta(left).timestamp.localeCompare(txMeta(right).timestamp))
  const latest = salaries.at(-1)
  if (!latest) return
  const automation: Automation = {
    id: uuid(),
    name: 'Зарплата',
    active: true,
    preset: null,
    schedule: { monthly: { day: new Date(txMeta(latest).timestamp).getDate() } },
    starts_at: txMeta(salaries[0]).timestamp,
    template: draftOf(structuredClone(latest)),
    last_planned_at: txMeta(latest).timestamp,
    created_at: iso(now()),
    amount_mode: 'varies',
    workday_shift: 'back',
    property_id: null,
    estimate: null,
  }
  automations.push(automation)
  syncWorkflow(automation)
  for (const tx of salaries) {
    const meta = txMeta(tx)
    const runId = uuid()
    meta.origin = {
      workflow: { workflow_id: automation.id, run_id: runId, planned_at: meta.timestamp, estimated: false },
    }
    const amount = 'topup' in tx ? tx.topup.amount : null
    runs.push({
      id: runId,
      automation_id: automation.id,
      planned_at: meta.timestamp,
      processed_at: meta.timestamp,
      outcome: { created: { tx_id: meta.id } },
      estimated: false,
      amount,
      symbol: 'topup' in tx ? tx.topup.asset.symbol : '',
    })
  }
}
