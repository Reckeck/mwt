/** Several linked operations from one form: a split convert or top-up. */

import { parseDecimal } from '@/shared/lib/decimal'
import type { TransactionFormState } from './form-state'
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
    out.push({ ...main, store_id: row.to.store, amount: row.receive.trim() })
  }
  return out
}

/** A decimal string with at most `places` digits after the point, no trailing zeros. */
function trimmed(value: number, places: number): string {
  return String(Number(value.toFixed(places)))
}

/**
 * One exchange paid out to several accounts: one operation per account, the
 * given amount split in proportion to what lands on each; the first takes
 * the rounding rest so the parts add up to exactly what was given.
 */
function splitConvert(form: TransactionFormState, main: Extract<TransactionInput, { type: 'convert' }>) {
  const rows = [{ to: form.to, receive: form.toAmount }, ...form.extra]
  const received = rows.map((row) => Number(parseDecimal(row.receive) ?? Number.NaN))
  const total = received.reduce((sum, value) => sum + value, 0)
  const given = Number(parseDecimal(form.fromAmount) ?? Number.NaN)
  if (!(total > 0 && given > 0) || received.some((value) => !(value > 0))) return null
  const places = Math.max(2, form.fromAmount.split(/[.,]/)[1]?.length ?? 0)
  const shares = received.map((value) => Number((given * (value / total)).toFixed(places)))
  shares[0] = Number((given - shares.slice(1).reduce((sum, value) => sum + value, 0)).toFixed(places))
  const out: TransactionInput[] = []
  for (const [index, row] of rows.entries()) {
    if (!row.to) return null
    out.push({
      ...main,
      from: { ...main.from, amount: trimmed(shares[index], places) },
      to: { ...main.to, account: row.to, amount: row.receive.trim() },
      rate_override: index === 0 ? main.rate_override : null,
    })
  }
  return out
}
