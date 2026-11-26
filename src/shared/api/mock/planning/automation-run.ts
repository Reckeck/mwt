/**
 * Automation runs with an operation template in the mock (spec op-period D5, D9, D11, I12): the period and
 * the rule of the template, the question of a run that asks, its confirmation and the preview of a rule.
 */

import { txId, txMeta } from '@/shared/lib/transactions'
import { mockBalanceOf, mockNotices, mockPush, mockTransactions } from '../demo'
import { runs as workflowRuns, waits } from '../workflows/state'
import { periodOfRule } from '../period'
import { previewAutomationDates } from './dates'
import { applyDistribution } from './distribution'
import {
  atomically,
  bookableRows,
  cardBeforeRows,
  overAllocated,
  placesOf,
  previewOf,
  resolveRule,
  text,
} from './distribution-rule'
import { automations, fail, iso, now, runs, uuid } from './state'
import { findTemplate } from './template-store'

interface Waiting {
  automationId: string
  income: Transaction
}
/** Runs that ask: nothing is booked until `confirm_distribution` (D9, Q12). */
export const waitingRuns = new Map<string, Waiting>()

const automationOf = (id: string) => automations.find((entry) => entry.id === id) ?? fail(`automation ${id}`, 'MISSING')

/** The template and the rule an automation distributes by. */
export function ruleOf(automation: Automation): { template: OperationTemplate; rule: DistributionRule } | null {
  const template = automation.operation_template_id ? findTemplate(automation.operation_template_id) : null
  return template?.distribution ? { template, rule: template.distribution } : null
}

/** What the automation's template books: the draft's income body or a refusal. */
function incomeOfAutomation(automation: Automation): TopupTx {
  const draft = automation.template as Transaction
  if (!('topup' in draft)) return fail('DISTRIBUTION_NOT_INCOME: only an income distributes')
  return draft.topup
}

/** D11: the rows of the next planned payout of an automation. */
export function previewRule(automationId: string, amount: string | null): DistributionPreview {
  const automation = automationOf(automationId)
  const template = automation.operation_template_id
    ? findTemplate(automation.operation_template_id)
    : fail("the automation's operation template", 'MISSING')
  const rule = template.distribution ?? fail("the template's distribution rule", 'MISSING')
  const plannedAt =
    previewAutomationDates({
      schedule: automation.schedule,
      starts_at: automation.starts_at,
      workday_shift: automation.workday_shift,
      time: automation.time ?? null,
    })[0] ?? fail('a next planned date of the automation', 'MISSING')
  const income = incomeOfAutomation(automation)
  const period = template.period_rule ? periodOfRule(template.period_rule, plannedAt.slice(0, 10)) : undefined
  const payout = Number(amount ?? income.amount)
  const resolved = resolveRule(
    rule,
    payout,
    Number(income.amount),
    mockBalanceOf(income.store_id, income.asset.symbol) + payout,
    placesOf(income.asset.symbol),
  )
  return previewOf(payout, resolved, { planned_at: plannedAt, period })
}

/** I12: «yes» to a run that asks: the income is booked at the confirmed amount and distributed. */
export function confirmDistribution(input: ConfirmDistributionInput): Transaction {
  if (input.mode === 'ask') fail('a confirmation chooses how to distribute, it cannot ask again')
  if (Number(input.amount) <= 0) fail('the amount must be above zero')
  const waiting = waitingRuns.get(input.run_id) ?? fail(`question of run ${input.run_id}`, 'MISSING')
  const automation = automationOf(waiting.automationId)
  const { template, rule } = ruleOf(automation) ?? fail("the template's distribution rule", 'MISSING')
  const tx = structuredClone(waiting.income)
  if (!('topup' in tx)) return fail('DISTRIBUTION_NOT_INCOME: only an income distributes')
  tx.topup.amount = input.amount
  const mode = input.mode ?? rule.on_other_amount
  if (mode === 'ask') fail('the rule still asks: choose how to distribute')
  const places = placesOf(tx.topup.asset.symbol)
  const resolved = resolveRule(
    { ...rule, on_other_amount: mode },
    Number(input.amount),
    Number(template.amount ?? incomeOfAutomation(automation).amount),
    cardBeforeRows(tx.topup, false),
    places,
  )
  if (resolved.over !== undefined) return overAllocated(resolved.over)
  const rows = bookableRows(resolved)
  atomically(() => {
    mockPush(tx)
    if (rows) applyDistribution(txId(tx), rows)
  })
  waitingRuns.delete(input.run_id)
  answerQuestion(input.run_id)
  runs.push({
    id: input.run_id,
    automation_id: automation.id,
    planned_at: iso(now()),
    processed_at: iso(now()),
    outcome: { created: { tx_id: txId(tx) } },
    estimated: false,
    amount: input.amount,
    symbol: tx.topup.asset.symbol,
  })
  return mockTransactions.find((stored) => txId(stored) === txId(tx)) ?? tx
}

/** D9, Q12: the question a run that asks leaves for the confirmation queue, keyed by the run id. */
function leaveQuestion(automation: Automation, income: TopupTx, runId: string): void {
  const stamp = iso(now())
  const question = `${automation.name}: ${income.amount} ${income.asset.symbol}`
  workflowRuns.unshift({
    id: runId,
    workflow_id: automation.id,
    trigger: 'income',
    started_at: stamp,
    outcome: { asking: { node_id: 'ask', question, until: null } },
    created: [],
    queued: 0,
    trace: [],
    dry_run: false,
  })
  waits.push({
    id: uuid(),
    workflow_id: automation.id,
    run_id: runId,
    node_id: 'ask',
    resume_at: stamp,
    memo: '[]',
    trigger: 'income',
    kind: 'confirm',
    question,
    created_at: stamp,
  })
}

/** I12: the answer closes the question of the run. */
function answerQuestion(runId: string): void {
  const index = waits.findIndex((entry) => entry.run_id === runId)
  if (index >= 0) waits.splice(index, 1)
  const run = workflowRuns.find((entry) => entry.id === runId)
  if (run) run.outcome = 'done'
}

/** Q11: the income is booked alone and a notice says the distribution did not happen. */
function noticeOver(automation: Automation, income: TopupTx, runId: string, over: number): void {
  const stamp = iso(now())
  mockNotices.push({
    key: `automation-undistributed:${runId}`,
    category: 'automation',
    title: `“${automation.name}” on ${income.meta.timestamp.slice(0, 10)}: not distributed`,
    body: `The rule asks ${text(over)} more than came; the income is booked undistributed`,
    link: '/automations',
    at: income.meta.timestamp,
    delivered_at: stamp,
    read: false,
  })
}

/**
 * D5, D9: a run of an automation with a template takes the template's period and distributes by its rule.
 * Books `tx` at the head of `list`; returns `true` when the rule asks and nothing is booked yet.
 */
export function bookRun(automation: Automation, tx: Transaction, runId: string, list: Transaction[]): boolean {
  const template = automation.operation_template_id ? findTemplate(automation.operation_template_id) : null
  const meta = txMeta(tx)
  if (template?.period_rule && ('topup' in tx || 'payment' in tx)) {
    meta.period = periodOfRule(template.period_rule, meta.timestamp.slice(0, 10))
  }
  const ruled = ruleOf(automation)
  if (!ruled || !('topup' in tx)) {
    list.unshift(tx)
    return false
  }
  const income = tx.topup
  const resolved = resolveRule(
    ruled.rule,
    Number(income.amount),
    Number(ruled.template.amount ?? incomeOfAutomation(automation).amount),
    cardBeforeRows(income, false),
    placesOf(income.asset.symbol),
  )
  // An automation whose rule asks always puts the question ahead of the run (core `rule_asks`).
  if (resolved.ask || ruled.rule.on_other_amount === 'ask') {
    waitingRuns.set(runId, { automationId: automation.id, income: tx })
    leaveQuestion(automation, income, runId)
    return true
  }
  if (resolved.over !== undefined) {
    list.unshift(tx)
    noticeOver(automation, income, runId, resolved.over)
    return false
  }
  const rows = bookableRows(resolved)
  atomically(() => {
    list.unshift(tx)
    if (rows) applyDistribution(meta.id, rows)
  })
  return false
}
