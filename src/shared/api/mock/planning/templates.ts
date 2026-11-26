/**
 * Operation templates in the mock (spec op-period T1-T5, I11-I13): the store with the core's
 * validation messages, quick-add, the preview of a rule and the confirmation of an asking run.
 */

import { txId, txMeta } from '@/shared/lib/transactions'
import { mockCategories, mockContacts, mockTransactionFrom } from '../ledger'
import { resolve } from '../core'
import type { MockCommands } from '../core'
import { mockBalanceOf, mockPush, mockStores, mockTransactions, priceOf } from '../demo'
import { periodOfRule } from '../period'
import { previewAutomationDates } from './dates'
import { applyDistribution } from './distribution'
import {
  bookableRows,
  checkRows,
  checkTargets,
  overAllocated,
  atomically,
  cardBeforeRows,
  placesOf,
  previewOf,
  resolveRule,
} from './distribution-rule'
import { automations, fail, iso, now, runs, uuid } from './state'

const NOT_POSITIVE = 'DISTRIBUTION_PART_NOT_POSITIVE'

/** Every template, live and archived. */
export const mockTemplates: OperationTemplate[] = []

const sameName = (left: string, right: string) => left.trim().toLowerCase() === right.trim().toLowerCase()

export function findTemplate(id: string): OperationTemplate {
  return mockTemplates.find((template) => template.id === id) ?? fail(`template ${id}`, 'MISSING')
}

/** T2: the rules of one template; its own record is not a duplicate of itself. */
function checkTemplate(template: OperationTemplate): void {
  if (!template.name.trim()) fail('name the template')
  const taken = mockTemplates.some(
    (known) => known.id !== template.id && !known.archived && !template.archived && sameName(known.name, template.name),
  )
  if (taken) fail(`a template named ${template.name.trim()} already exists`)
  for (const key of template.categories) {
    if (!mockCategories.some((category) => category.key === key)) fail(`category ${key}`, 'MISSING')
  }
  if (template.store_id && !mockStores.some((store) => store.id === template.store_id)) {
    fail(`store ${template.store_id}`, 'MISSING')
  }
  if (template.contact_id && !mockContacts.some((contact) => contact.id === template.contact_id)) {
    fail(`contact ${template.contact_id}`, 'MISSING')
  }
  const rule = template.distribution
  if (!rule) return
  if (template.kind !== 'income') fail('only an income template can carry a distribution')
  checkRows(rule.rows, false)
  for (const row of rule.rows) {
    const asked = typeof row.kind === 'object' ? ('sum' in row.kind ? row.kind.sum.amount : row.kind.percent.pct) : '1'
    if (Number(asked) <= 0) fail(`${NOT_POSITIVE}: a row of ${asked} is not allowed`)
  }
  checkTargets(rule.rows)
}

function made(input: OperationTemplateInput, base: Pick<OperationTemplate, 'id' | 'archived' | 'created_at'>) {
  const template: OperationTemplate = { ...base, ...input, name: input.name.trim() }
  checkTemplate(template)
  return template
}

function replaced(template: OperationTemplate): OperationTemplate {
  mockTemplates.splice(
    mockTemplates.findIndex((known) => known.id === template.id),
    1,
    template,
  )
  return template
}

/** What the booked income of a template looks like before it is stored (T4, D3). */
function incomeInput(template: OperationTemplate, input: CreateFromTemplateInput): TransactionInput {
  const storeId = input.store_id ?? template.store_id ?? fail('TEMPLATE_NO_STORE: pick a store')
  const asset = template.asset ?? fail('TEMPLATE_NO_ASSET: the template has no asset')
  const amount = input.amount ?? template.amount ?? fail('TEMPLATE_NO_AMOUNT: enter an amount')
  const common = {
    timestamp: input.timestamp || null,
    comment: input.comment ?? template.comment,
    categories: template.categories,
    flow_id: null,
    contact_id: template.contact_id ?? null,
    price_usd: null,
    fx_override: null,
    price: null,
    price_currency: null,
    // The template's own rule is applied below, after the category's rule is kept out of the way.
    no_period_rule: Boolean(input.no_period_rule || template.period_rule),
  } as TxCommonInput
  const ident = { ...asset, name: asset.name ?? null } as AssetIdent
  const body = { store_id: storeId, asset: ident, amount, fee: null, details: {} as KindDetails }
  return template.kind === 'income'
    ? ({ type: 'topup', common, ...body, from: null, parts: [] } as TransactionInput)
    : ({ type: 'payment', common, ...body, to: null } as TransactionInput)
}

/** T4: the operation of a template; an income with a distribution rule is distributed in the same write. */
function quickAdd(input: CreateFromTemplateInput): TemplateOperation {
  const template = findTemplate(input.template_id)
  const draft = incomeInput(template, input)
  const tx = mockTransactionFrom(draft, priceOf, undefined, mockBalanceOf)
  const meta = txMeta(tx)
  if (template.period_rule && !input.no_period_rule)
    meta.period = periodOfRule(template.period_rule, meta.timestamp.slice(0, 10))
  if (!('topup' in tx) || !template.distribution) {
    mockPush(tx)
    return { transaction: tx, ask: false }
  }
  const income = tx.topup
  const expected = Number(template.amount ?? income.amount)
  const resolved = resolveRule(
    template.distribution,
    Number(income.amount),
    expected,
    cardBeforeRows(income, false),
    placesOf(income.asset.symbol),
  )
  if (resolved.ask) {
    mockPush(tx)
    return { transaction: tx, ask: true }
  }
  if (resolved.over !== undefined) return overAllocated(resolved.over)
  const rows = bookableRows(resolved)
  atomically(() => {
    mockPush(tx)
    if (rows) applyDistribution(txId(tx), rows)
  })
  return { transaction: tx, ask: false }
}

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
function previewRule(automationId: string, amount: string | null): DistributionPreview {
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
function confirmDistribution(input: ConfirmDistributionInput): Transaction {
  if (input.mode === 'ask') fail('a confirmation chooses how to distribute, it cannot ask again')
  if (Number(input.amount) <= 0) fail('the amount must be above zero')
  const waiting = waitingRuns.get(input.run_id) ?? fail(`question of run ${input.run_id}`, 'MISSING')
  const automation = automationOf(waiting.automationId)
  const { template, rule } = ruleOf(automation) ?? fail("the template's distribution rule", 'MISSING')
  const tx = structuredClone(waiting.income)
  if (!('topup' in tx)) return fail('DISTRIBUTION_NOT_INCOME: only an income distributes')
  tx.topup.amount = input.amount
  const mode = input.mode ?? rule.on_other_amount
  if (mode === 'ask') fail('the rule asks again: choose how to distribute')
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
  if (resolved.ask) {
    waitingRuns.set(runId, { automationId: automation.id, income: tx })
    return true
  }
  if (resolved.over !== undefined) return overAllocated(resolved.over)
  const rows = bookableRows(resolved)
  atomically(() => {
    list.unshift(tx)
    if (rows) applyDistribution(meta.id, rows)
  })
  return false
}

/** T3: a template is deleted only while no automation refers to it. */
function deleteTemplate(id: string): OperationTemplate {
  const template = findTemplate(id)
  const user = automations.find((automation) => automation.operation_template_id === id)
  if (user) fail(`template ${template.name} is used by the automation ${user.name}; archive it instead`)
  mockTemplates.splice(mockTemplates.indexOf(template), 1)
  return template
}

export const templateCommands: MockCommands = {
  list_operation_templates: () =>
    resolve([...mockTemplates].sort((left, right) => left.name.toLowerCase().localeCompare(right.name.toLowerCase()))),
  create_operation_template: ({ body }) => {
    const template = made(body as OperationTemplateInput, { id: uuid(), archived: false, created_at: iso(now()) })
    mockTemplates.push(template)
    return resolve(template)
  },
  update_operation_template: ({ body }) => {
    const input = body as UpdateOperationTemplateInput
    const old = findTemplate(input.id)
    return resolve(replaced(made(input.template, old)))
  },
  archive_operation_template: ({ args }) => {
    const template = { ...findTemplate(String(args?.id)), archived: Boolean(args?.archived) }
    if (!template.archived) checkTemplate(template)
    return resolve(replaced(template))
  },
  delete_operation_template: ({ args }) => resolve(deleteTemplate(String(args?.id))),
  create_transaction_from_template: ({ body }) => resolve(quickAdd(body as CreateFromTemplateInput)),
  preview_distribution_rule: ({ args }) =>
    resolve(previewRule(String(args?.automationId), args?.amount ? String(args.amount) : null)),
  confirm_distribution: ({ body }) => resolve(confirmDistribution(body as ConfirmDistributionInput)),
}
