/** Several linked operations from one form: a split convert or top-up. */

import { decimalAdd, decimalCmp, decimalShare, decimalSub, parseDecimal } from '@/shared/lib/decimal'
import type { TransactionFormState } from './form-state'
import { splitBalanced, splitTotals } from './form-helpers'
import { formToInput } from './to-input'

/** Kinds that may be split between several recipients. */
export const SPLITTABLE: TxKind[] = ['convert', 'topup']

/** Whether the form saves as several linked operations. */
export function isLinked(form: TransactionFormState): boolean {
  return SPLITTABLE.includes(form.kind) && form.extra.length > 0
}

/**
 * The main operation and one per extra recipient: an exchange gives each its
 * share of the same asset from the same account; a top-up puts the same
 * asset on another account. `null` while something required is missing.
 */
export function formToInputs(form: TransactionFormState): TransactionInput[] | null {
  const main = formToInput(form)
  if (!main) return null
  if (!isLinked(form)) return [main]
  if (main.type === 'convert') return splitConvert(form, main)
  const out: TransactionInput[] = [main]
  for (const row of form.extra) {
    if (main.type !== 'topup' || !row.to || !('store' in row.to)) return null
    out.push({
      ...main,
      store_id: row.to.store,
      amount: row.receive.trim(),
      fee: null,
      details: { ...main.details, tx_hash: null },
    })
  }
  return out
}

/**
 * One exchange paid out to several accounts: one operation per account, the
 * given amount split in proportion to what lands on each; the first takes
 * the rounding rest so the parts add up to exactly what was given. `toAmount`
 * is the total received; the first account gets `toShare`, or what the extra
 * rows leave. The fee belongs to the exchange, so only the first part carries
 * it. `null` while the shares do not add up to the total.
 */
function splitConvert(form: TransactionFormState, main: Extract<TransactionInput, { type: 'convert' }>) {
  const totals = splitTotals(form)
  if (!totals || !splitBalanced(totals)) return null
  const extraSum = form.extra.reduce<string | null>(
    (sum, row) => (sum === null ? null : decimalAdd(sum, row.receive)),
    '0',
  )
  if (extraSum === null) return null
  const first = form.toShare.trim() ? form.toShare.trim() : decimalSub(totals.total, extraSum)
  if (first === null) return null
  const rows = [{ to: form.to, receive: first }, ...form.extra]
  const total = rows.reduce<string | null>((sum, row) => (sum === null ? null : decimalAdd(sum, row.receive)), '0')
  const given = parseDecimal(form.fromAmount)
  const positive = (value: string | null) => value !== null && decimalCmp(value, '0') === 1
  if (total === null || given === null || !positive(total) || !positive(given)) return null
  if (!rows.every((row) => positive(row.receive))) return null
  const places = Math.max(2, given.split('.')[1]?.length ?? 0)
  const shares = rows.map((row) => decimalShare(given, row.receive, total, places))
  const rest = shares
    .slice(1)
    .reduce<string | null>((sum, value) => (sum === null || value === null ? null : decimalAdd(sum, value)), '0')
  const head = rest === null ? null : decimalSub(given, rest)
  if (head === null || shares.some((value) => value === null)) return null
  shares[0] = head
  const out: TransactionInput[] = []
  for (const [index, row] of rows.entries()) {
    if (!row.to) return null
    out.push({
      ...main,
      from: { ...main.from, amount: shares[index] as string },
      to: { ...main.to, account: row.to, amount: row.receive.trim() },
      rate_override: index === 0 ? main.rate_override : null,
      fee: index === 0 ? main.fee : null,
    })
  }
  return out
}
