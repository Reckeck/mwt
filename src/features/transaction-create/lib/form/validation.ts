/** What the form still lacks before it can be an operation. */

import { parseDecimal } from '@/shared/lib/decimal'
import type { TransactionFormState } from './form-state'
import { splitBalanced, splitTotals } from './form-helpers'
import { SPLITTABLE, formToInputs } from './linked'

export { splitTotals }

/**
 * What still has to be filled before the form can be saved, as i18n keys
 * (`tx.missing.*`) — said under the form instead of a silently grey button.
 */
export function missingFields(form: TransactionFormState): string[] {
  const out: string[] = []
  const blank = (value: string) => parseDecimal(value) === null || Number(value.replace(',', '.')) <= 0
  switch (form.kind) {
    case 'convert':
      if (!form.from) out.push('from')
      if (!form.fromAsset) out.push('fromAsset')
      if (blank(form.fromAmount)) out.push('fromAmount')
      if (form.viaOpen && !form.via) out.push('via')
      if (!form.to) out.push('to')
      if (!form.toAsset) out.push('toAsset')
      if (blank(form.toAmount)) out.push('toAmount')
      break
    case 'transfer':
      if (!form.asset) out.push('asset')
      if (blank(form.amount)) out.push('amount')
      if (!form.from) out.push('from')
      if (!form.to) out.push('to')
      break
    case 'trade_spot':
      if (!form.soldAsset) out.push('soldAsset')
      if (blank(form.soldQty)) out.push('soldQty')
      if (!form.boughtAsset) out.push('boughtAsset')
      if (blank(form.boughtQty)) out.push('boughtQty')
      break
    case 'trade_futures':
      if (!form.symbol.trim()) out.push('symbol')
      if (blank(form.quantity)) out.push('quantity')
      if (blank(form.fillPrice)) out.push('fillPrice')
      break
    case 'asset_sell':
      if (!form.asset) out.push('asset')
      if (blank(form.sellQty)) out.push('sellQty')
      if (blank(form.sellPrice)) out.push('sellPrice')
      if (!form.currency) out.push('currency')
      break
    case 'adjustment':
      break
    default:
      if (!form.asset) out.push('asset')
      if (blank(form.amount)) out.push('amount')
  }
  if (SPLITTABLE.includes(form.kind) && form.extra.some((row) => !row.to || blank(row.receive))) out.push('extra')
  const totals = splitTotals(form)
  if (totals && !splitBalanced(totals)) out.push('shares')
  return out
}

/** Whether every field the kind cannot do without is filled (the backend checks the rest). */
export function isFormComplete(form: TransactionFormState): boolean {
  if (!formToInputs(form) || missingFields(form).length) return false
  const filled = (...values: string[]) => values.every((value) => value.trim() !== '')
  switch (form.kind) {
    case 'convert':
      return filled(form.fromAmount, form.toAmount)
    case 'trade_spot':
      return filled(form.soldQty, form.boughtQty)
    case 'trade_futures':
      return filled(form.symbol, form.quantity, form.fillPrice)
    case 'asset_sell':
      return filled(form.sellPrice, form.sellQty)
    default:
      return filled(form.amount)
  }
}
