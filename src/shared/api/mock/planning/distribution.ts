/**
 * Income distribution in the mock (spec op-period I8-I10, S6-S11, D8-D10): the same arithmetic and
 * the same messages as the core's `planning::distribution` and `usecases::planning::distribution`.
 */

import { txId, txMeta } from '@/shared/lib/transactions'
import { mockEnvelopeView, mockEnvelopes, recordEnvelopeMove } from '../catalog'
import { resolve } from '../core'
import type { MockCommands } from '../core'
import { mockBalanceOf, mockPush, mockTransactions, priceOf } from '../demo'
import { mockTransactionFrom } from '../ledger'
import {
  BAD_TARGET,
  PART_NOT_POSITIVE,
  atomically,
  cardBeforeRows,
  checkTargets,
  isRemainder,
  overAllocated,
  placesOf,
  previewOf,
  resolveRows,
  resolveRule,
  text,
} from './distribution-rule'
import { fail, goals, iso, now, uuid } from './state'

const ALREADY_DISTRIBUTED = 'DISTRIBUTION_ALREADY_DISTRIBUTED'
const NOT_INCOME = 'DISTRIBUTION_NOT_INCOME'
const WITH_PARTS = 'DISTRIBUTION_WITH_PARTS'
const LINKED_TO_INCOME = 'LINKED_TO_INCOME'

/** The refusal of a change to a record an income's distribution owns. */
const linked = (what: string) =>
  fail(`${LINKED_TO_INCOME}: ${what} belongs to an income distribution; change it with replace_distribution`)

function incomeBody(id: string): TopupTx {
  const found = mockTransactions.find((tx) => txId(tx) === id)
  if (!found) return fail(`income ${id}`, 'MISSING')
  if (!('topup' in found)) return fail(`${NOT_INCOME}: only an income can be distributed`)
  return found.topup
}

const incomeOfSource = (source: IncomeSource): { income: TopupTx; stored: boolean } => {
  if (source.source === 'stored') return { income: incomeBody(source.id), stored: true }
  const draft = mockTransactionFrom(source.transaction, priceOf, undefined, mockBalanceOf)
  if (!('topup' in draft)) return fail('only an income distributes')
  return { income: draft.topup, stored: false }
}

const sameTarget = (left: DistTarget, right: DistTarget) => JSON.stringify(left) === JSON.stringify(right)

function dropEffect(effect: EffectRef): void {
  if ('transfer' in effect) {
    const index = mockTransactions.findIndex((tx) => txId(tx) === effect.transfer.tx_id)
    if (index >= 0) mockTransactions.splice(index, 1)
    return
  }
  const envelope = mockEnvelopes.find((entry) => entry.id === effect.envelope_move.envelope_id)
  if (!envelope) return
  envelope.moves = envelope.moves.filter((move) => move.id !== effect.envelope_move.move_id)
  envelope.allocated = String(envelope.moves.reduce((sum, move) => sum + Number(move.delta), 0))
}

function goalEnvelope(income: TopupTx, goalId: string, storeId: string): Envelope {
  if (storeId !== income.store_id) {
    fail(`${BAD_TARGET}: a goal row can only be funded on the card the income arrived on`)
  }
  const found = mockEnvelopes.find((entry) => entry.store_id === storeId && entry.goal_id === goalId)
  if (found) return found
  const goal = goals.find((entry) => entry.id === goalId)
  const made: Envelope = {
    id: uuid(),
    store_id: storeId,
    name: goal?.name ?? '',
    asset: income.asset,
    allocated: '0',
    moves: [],
    target: null,
    goal_id: goalId,
    created_at: iso(now()),
  }
  mockEnvelopes.push(made)
  return made
}

function createEffect(income: TopupTx, part: DistributionPart): EffectRef {
  const amount = Number(part.amount)
  const at = income.meta.timestamp
  const target = part.target
  if (target !== 'none' && 'store' in target) {
    const symbol = income.asset.symbol
    const free = mockBalanceOf(income.store_id, symbol)
    if (free < amount) fail(`only ${text(free)} ${symbol} is available on the card`, 'INSUFFICIENT_FUNDS')
    const transfer: Transaction = {
      transfer: {
        meta: { ...income.meta, id: uuid(), comment: '', categories: [], period: undefined, spread: undefined },
        from: { store: income.store_id },
        to: { store: target.store.id },
        asset: income.asset,
        amount: part.amount,
        fee: null,
        network: null,
        tx_hash: null,
        debt: null,
        price: income.price,
      },
    }
    const meta = txMeta(transfer)
    meta.origin = 'manual'
    meta.income_ref = income.meta.id
    delete meta.period
    delete meta.spread
    mockPush(transfer)
    return { transfer: { tx_id: meta.id } }
  }
  const envelope =
    target !== 'none' && 'goal' in target
      ? goalEnvelope(income, target.goal.goal_id, target.goal.store_id)
      : mockEnvelopes.find((entry) => target !== 'none' && 'envelope' in target && entry.id === target.envelope.id)
  if (!envelope) return fail('only the remainder may stay on the card')
  if (envelope.store_id !== income.store_id || envelope.asset.symbol !== income.asset.symbol) {
    fail('the envelope must sit on the card and the asset of the income')
  }
  const free = Number(mockEnvelopeView(envelope).free)
  if (free < amount) fail(`only ${text(free)} ${envelope.asset.symbol} is free in this wallet`, 'INSUFFICIENT_FUNDS')
  const move = recordEnvelopeMove(envelope, amount, { incomeRef: income.meta.id, at })
  return { envelope_move: { envelope_id: envelope.id, move_id: move?.id ?? '' } }
}

/** S11: the list `rows` of `income` (or none): dropped or changed parts lose their effects, new ones get them. */
function rewrite(incomeId: string, rows: DistributionRow[] | null): DistributionPart[] {
  const income = incomeBody(incomeId)
  if (income.parts?.length) fail(`${WITH_PARTS}: an income cannot arrive in parts and distribute`)
  const places = placesOf(income.asset.symbol)
  const { amounts, over } = rows ? resolveRows(Number(income.amount), rows, places) : { amounts: [], over: undefined }
  if (over !== undefined) overAllocated(over)
  if (rows) {
    amounts.forEach((amount, index) => {
      if (!isRemainder(rows[index].kind) && amount <= 0) {
        fail(`${PART_NOT_POSITIVE}: a part of ${text(amount)} is not allowed`)
      }
    })
    checkTargets(rows)
  }
  const wanted: DistributionPart[] = (rows ?? []).map((row, index) => ({
    kind: row.kind,
    target: row.target,
    amount: text(amounts[index]),
    status: 'done',
  }))
  const reusable = (income.distribution ?? []).filter((part) => part.effect)
  const parts = wanted.map((part) => {
    if (isRemainder(part.kind)) return part
    const at = reusable.findIndex((old) => sameTarget(old.target, part.target) && old.amount === part.amount)
    if (at < 0) return part
    return { ...part, effect: reusable.splice(at, 1)[0].effect }
  })
  for (const old of reusable) if (old.effect) dropEffect(old.effect)
  for (const part of parts) if (!part.effect && !isRemainder(part.kind)) part.effect = createEffect(income, part)
  if (rows) income.distribution = parts
  else delete income.distribution
  return parts
}

/** S8: distribute an income that does not distribute yet; all or nothing. */
export function applyDistribution(incomeId: string, rows: DistributionRow[]): void {
  if (incomeBody(incomeId).distribution) fail(`${ALREADY_DISTRIBUTED}: replace the distribution instead`)
  atomically(() => rewrite(incomeId, rows))
}

const rowsOf = (parts: DistributionPart[]): DistributionRow[] =>
  parts.map((part) => ({ kind: part.kind, target: part.target }))

export function incomeTree(incomeId: string): IncomeTree {
  const income = incomeBody(incomeId)
  const found = mockTransactions.find((tx) => txId(tx) === incomeId) as Transaction
  const parts = (income.distribution ?? []).map((part): IncomePart => {
    const effect = part.effect
    if (effect && 'transfer' in effect) {
      return { part, transfer: mockTransactions.find((tx) => txId(tx) === effect.transfer.tx_id) }
    }
    if (!effect) return { part }
    const envelope = mockEnvelopes.find((entry) => entry.id === effect.envelope_move.envelope_id)
    return { part, envelope_move: envelope?.moves.find((move) => move.id === effect.envelope_move.move_id) }
  })
  return { income: found, parts }
}

function distributedOf(incomeId: string): DistributedIncome {
  const tree = incomeTree(incomeId)
  const transfers = tree.parts.flatMap((entry) => (entry.transfer ? [entry.transfer] : []))
  const envelopeMoves = tree.parts.flatMap((entry): EnvelopeMoveRef[] =>
    entry.part.effect && 'envelope_move' in entry.part.effect
      ? [{ ...entry.part.effect.envelope_move, amount: entry.part.amount }]
      : [],
  )
  return { income: tree.income, transfers, envelope_moves: envelopeMoves }
}

/** S11 guard: a record an income owns is not deleted on its own; an income takes its effects with it. */
export function prepareDelete(id: string): void {
  const found = mockTransactions.find((tx) => txId(tx) === id)
  if (!found) return
  if (txMeta(found).income_ref) linked('this operation')
  if ('topup' in found && found.topup.distribution) atomically(() => rewrite(id, null))
}

const effectShape = (tx: Transaction) =>
  'transfer' in tx
    ? JSON.stringify([tx.transfer.from, tx.transfer.to, tx.transfer.asset.symbol, tx.transfer.amount])
    : null

/**
 * S11 guard of an edit: the links the form has no field for stay, an effect keeps what V2 reads, an
 * income that moves what its effects hang on re-resolves its rows. Swaps `next` in for `stored`.
 */
export function storeEdited(stored: Transaction, next: Transaction): void {
  const old = txMeta(stored)
  if (old.income_ref && effectShape(stored) !== effectShape(next)) {
    linked('the amount, accounts and asset of this operation')
  }
  if (old.income_ref) txMeta(next).income_ref = old.income_ref
  const oldIncome = 'topup' in stored ? stored.topup : null
  if (oldIncome?.distribution && !('topup' in next)) linked('an income with a distribution')
  if (!oldIncome?.distribution && 'topup' in next && next.topup.distribution) {
    linked('an operation that is not an income')
  }
  const swap = () => mockTransactions.splice(mockTransactions.indexOf(stored), 1, next)
  if (!oldIncome?.distribution || !('topup' in next)) return void swap()
  const parts = oldIncome.distribution
  const moved =
    oldIncome.amount !== next.topup.amount ||
    oldIncome.store_id !== next.topup.store_id ||
    oldIncome.asset.symbol !== next.topup.asset.symbol ||
    oldIncome.meta.timestamp !== next.topup.meta.timestamp
  if (!moved) {
    next.topup.distribution = parts
    return void swap()
  }
  atomically(() => {
    rewrite(old.id, null)
    swap()
    rewrite(old.id, rowsOf(parts))
  })
}

export const distributionCommands: MockCommands = {
  preview_distribution: ({ body }) => {
    const input = body as PreviewDistributionInput
    const { income, stored } = incomeOfSource(input.income)
    const rule: DistributionRule = {
      rows: input.rows,
      on_other_amount: 'shrink_remainder',
      keep_on_card_min: input.keep_on_card_min,
    }
    const amount = Number(income.amount)
    const resolved = resolveRule(rule, amount, amount, cardBeforeRows(income, stored), placesOf(income.asset.symbol))
    return resolve(previewOf(amount, resolved))
  },
  distribute_income: ({ body }) => {
    const input = body as DistributeIncomeInput
    const source = input.income
    if (source.source === 'stored') {
      applyDistribution(source.id, input.rows)
      return resolve(distributedOf(source.id))
    }
    const draft = mockTransactionFrom(source.transaction, priceOf, undefined, mockBalanceOf)
    if (!('topup' in draft)) return fail('only an income distributes')
    atomically(() => {
      mockPush(draft)
      rewrite(txId(draft), input.rows)
    })
    return resolve(distributedOf(txId(draft)))
  },
  get_income_distribution: ({ args }) => resolve(incomeTree(String(args?.incomeId))),
  update_distribution: ({ body }) => {
    const input = body as UpdateDistributionInput
    atomically(() => rewrite(input.income_id, input.rows))
    return resolve(incomeTree(input.income_id))
  },
  undo_distribution: ({ args }) => {
    const id = String(args?.incomeId)
    atomically(() => rewrite(id, null))
    return resolve(incomeTree(id).income)
  },
}
