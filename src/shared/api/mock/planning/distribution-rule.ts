/** Pure arithmetic of a distribution (spec op-period S6, S7, D8-D10): rows resolved, a rule against a payout. */

import { isFiatCode } from '@/shared/lib/format'
import { mockEnvelopes } from '../catalog'
import { mockBalanceOf, mockStores, mockTransactions } from '../demo'
import { fail, goals } from './state'

const OVER_ALLOCATED = 'DISTRIBUTION_OVER_ALLOCATED'
const REMAINDER_COUNT = 'DISTRIBUTION_REMAINDER_COUNT'
export const PART_NOT_POSITIVE = 'DISTRIBUTION_PART_NOT_POSITIVE'
export const BAD_TARGET = 'DISTRIBUTION_BAD_TARGET'

export const placesOf = (symbol: string) => (isFiatCode(symbol) ? 2 : 8)
export const round = (value: number, places: number) => Number(value.toFixed(places))
export const text = (value: number) => String(value)

export const isRemainder = (kind: DistKind) => kind === 'remainder'

/** D7, S6: exactly one remainder, a sum or percent row asks for something. */
export function checkRows(rows: DistributionRow[], positive = false): void {
  const count = rows.filter((row) => isRemainder(row.kind)).length
  if (count !== 1) fail(`${REMAINDER_COUNT}: a distribution needs exactly one remainder, got ${count}`)
  if (!positive) return
  for (const row of rows) {
    const asked = typeof row.kind === 'object' ? ('sum' in row.kind ? row.kind.sum.amount : row.kind.percent.pct) : '1'
    if (Number(asked) <= 0) fail(`${PART_NOT_POSITIVE}: a row of ${asked} is not allowed`)
  }
}

/** S6, S14: targets that exist and are own; `none` only on the remainder. */
export function checkTargets(rows: DistributionRow[]): void {
  const bad = (message: string) => fail(`${BAD_TARGET}: ${message}`)
  const store = (id: string) => {
    const found = mockStores.find((entry) => entry.id === id)
    if (!found || found.archived) fail(`store ${id}`, 'MISSING')
  }
  for (const row of rows) {
    const target = row.target
    if (isRemainder(row.kind)) {
      if (target !== 'none') bad('the remainder stays on the card')
    } else if (target === 'none') bad('only the remainder may stay on the card')
    else if ('store' in target) store(target.store.id)
    else if ('envelope' in target) {
      const envelope = mockEnvelopes.find((entry) => entry.id === target.envelope.id)
      if (!envelope) fail(`envelope ${target.envelope.id}`, 'MISSING')
      else store(envelope.store_id)
    } else {
      const goal = goals.find((entry) => entry.id === target.goal.goal_id)
      if (!goal) fail(`goal ${target.goal.goal_id}`, 'MISSING')
      else if (!goal.active) bad('the goal is not active')
      store(target.goal.store_id)
      const own = goal?.scope.store_ids ?? []
      if (own.length && !own.includes(target.goal.store_id)) bad("the store is not the goal's own store")
    }
  }
}

interface ResolvedRows {
  amounts: number[]
  over?: number
}

/** S6, S7: sum and percent rows rounded to `places`, the remainder takes the rest (never negative). */
export function resolveRows(payout: number, rows: DistributionRow[], places: number): ResolvedRows {
  checkRows(rows)
  const asked = rows.map((row) =>
    typeof row.kind === 'string'
      ? null
      : 'sum' in row.kind
        ? Number(row.kind.sum.amount)
        : round((payout * Number(row.kind.percent.pct)) / 100, places),
  )
  const left = round(payout - asked.reduce<number>((sum, amount) => sum + (amount ?? 0), 0), places)
  const amounts = asked.map((amount) => amount ?? Math.max(left, 0))
  return left < 0 ? { amounts, over: -left } : { amounts }
}

export interface ResolvedRule {
  rows: { row: DistributionRow; amount: number }[]
  remainder: number
  over?: number
  ask: boolean
  cut: { row_index: number; by: number }[]
}

const nothing = (patch: Partial<ResolvedRule>): ResolvedRule => ({
  rows: [],
  remainder: 0,
  ask: false,
  cut: [],
  ...patch,
})

/** D8-D10: a rule against the actual payout, the expected one and the card as held before the rows. */
export function resolveRule(
  rule: DistributionRule,
  actual: number,
  expected: number,
  cardBeforeRows: number,
  places: number,
): ResolvedRule {
  const differs = actual !== expected
  if (differs && rule.on_other_amount === 'ask') return nothing({ ask: true })
  const scale = differs && rule.on_other_amount === 'shrink_all' && expected !== 0
  const asked = rule.rows.map((row): DistributionRow => {
    if (!scale || typeof row.kind === 'string' || !('sum' in row.kind)) return row
    const amount = round((Number(row.kind.sum.amount) * actual) / expected, places)
    return { kind: { sum: { amount: text(amount) } }, target: row.target }
  })
  const { amounts, over } = resolveRows(actual, asked, places)
  if (over !== undefined) return nothing({ over })
  const rows = asked.map((row, index) => ({ row, amount: amounts[index] }))
  const leaving = rows.filter((entry) => !isRemainder(entry.row.kind)).reduce((sum, entry) => sum + entry.amount, 0)
  let missing = rule.keep_on_card_min ? Number(rule.keep_on_card_min) - (cardBeforeRows - leaving) : 0
  const cut: ResolvedRule['cut'] = []
  for (let index = rows.length - 1; index >= 0 && missing > 0; index--) {
    const entry = rows[index]
    if (isRemainder(entry.row.kind) || entry.amount <= 0) continue
    const by = Math.min(entry.amount, missing)
    entry.amount = round(entry.amount - by, places)
    missing = round(missing - by, places)
    cut.push({ row_index: index, by })
  }
  const remainder = round(
    cut.reduce((sum, entry) => sum + entry.by, 0) + (rows.find((entry) => isRemainder(entry.row.kind))?.amount ?? 0),
    places,
  )
  for (const entry of rows) if (isRemainder(entry.row.kind)) entry.amount = remainder
  return { rows, remainder, ask: false, cut }
}

/** `DistributionPreview` of a resolved rule: rows empty on `over` and `ask`. */
export function previewOf(
  amount: number,
  resolved: ResolvedRule,
  extra?: { planned_at: string; period?: OpPeriod },
): DistributionPreview {
  return {
    amount: text(amount),
    ...extra,
    rows: resolved.rows.map((entry) => ({ row: entry.row, amount: text(entry.amount) })),
    remainder: text(resolved.remainder),
    ...(resolved.over === undefined ? {} : { over: text(resolved.over) }),
    ask: resolved.ask,
    cut: resolved.cut.map((entry) => ({ row_index: entry.row_index, by: text(entry.by) })),
  }
}

/** T14: the rows a rule books for an actual payout; rows cut to zero are dropped, a cut row becomes a sum. */
export function bookableRows(resolved: ResolvedRule): DistributionRow[] | null {
  const cutAt = new Set(resolved.cut.map((entry) => entry.row_index))
  const rows = resolved.rows
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => isRemainder(entry.row.kind) || entry.amount > 0)
    .map(({ entry, index }): DistributionRow => {
      if (isRemainder(entry.row.kind) || !cutAt.has(index)) return entry.row
      return { kind: { sum: { amount: text(entry.amount) } }, target: entry.row.target }
    })
  return rows.some((row) => !isRemainder(row.kind)) ? rows : null
}

/** The refusal of a rule whose rows ask more than the income. */
export const overAllocated = (over: number) =>
  fail(`${OVER_ALLOCATED}: the rows ask ${text(over)} more than the income`)

/** The card as the rows meet it: what it holds before any row, transfers of a stored distribution added back. */
export function cardBeforeRows(income: TopupTx, stored: boolean): number {
  const held = mockBalanceOf(income.store_id, income.asset.symbol)
  if (!stored) return held + Number(income.amount)
  const moved = (income.distribution ?? [])
    .filter((part) => part.effect && 'transfer' in part.effect)
    .reduce((sum, part) => sum + Number(part.amount), 0)
  return held + moved
}

/** Run `change`; on any error the ledger and the envelopes are put back as they were. */
export function atomically<T>(change: () => T): T {
  const ledger = structuredClone(mockTransactions)
  const envelopes = structuredClone(mockEnvelopes)
  try {
    return change()
  } catch (error) {
    mockTransactions.splice(0, mockTransactions.length, ...ledger)
    mockEnvelopes.splice(0, mockEnvelopes.length, ...envelopes)
    throw error
  }
}
