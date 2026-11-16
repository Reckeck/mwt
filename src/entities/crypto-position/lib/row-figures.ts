import { decimalAdd, parseDecimal } from '@/shared/lib/decimal'
import { type MissingReason, accruesOnBalance, feesApply, priceReason } from './breakdown'

export interface RowFigure {
  amount: string | null
  reason?: MissingReason
}

const known = (value: string | undefined): string | null => (value === undefined ? null : parseDecimal(value))

/**
 * The fees collected and not yet collected. A pool always has them; a supply or a stake only once rewards were
 * claimed or are pending — the same rule as the detail's breakdown, so the column and the breakdown agree.
 */
export function feesFigure(position: CryptoPositionView): RowFigure {
  if (!feesApply(position)) return { amount: null, reason: 'notApplicable' }
  const collected = known(position.fees_collected_usd)
  const uncollected = known(position.fees_uncollected_usd)
  if (collected === null || uncollected === null) return { amount: null, reason: 'notRead' }
  return { amount: decimalAdd(collected, uncollected) }
}

/** What a supply or a stake accrued on its own balance; a pool has no such line. */
export function accrualFigure(position: CryptoPositionView): RowFigure {
  if (!accruesOnBalance(position.kind)) return { amount: null, reason: 'notApplicable' }
  const accrued = known(position.accrued_usd)
  return accrued === null ? { amount: null, reason: priceReason(position) } : { amount: accrued }
}

/** The worth now; a closed position is a real zero, which is a figure. */
export function valueFigure(position: CryptoPositionView): RowFigure {
  const value = known(position.value_usd)
  return value === null ? { amount: null, reason: priceReason(position) } : { amount: value }
}
