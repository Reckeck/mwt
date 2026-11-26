/** Operation templates in the mock (spec op-period T4, I13): quick-add and the template commands. */

import { txId, txMeta } from '@/shared/lib/transactions'
import { mockTransactionFrom } from '../ledger'
import { resolve } from '../core'
import type { MockCommands } from '../core'
import { mockBalanceOf, mockPush, priceOf } from '../demo'
import { periodOfRule } from '../period'
import { confirmDistribution, previewRule } from './automation-run'
import { applyDistribution } from './distribution'
import { atomically, bookableRows, cardBeforeRows, overAllocated, placesOf, resolveRule } from './distribution-rule'
import { fail, iso, now, uuid } from './state'
import { checkTemplate, deleteTemplate, findTemplate, made, mockTemplates, replaced } from './template-store'

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
