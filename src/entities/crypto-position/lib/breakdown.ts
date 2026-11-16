import { decimalAdd, decimalSub, parseDecimal } from '@/shared/lib/decimal'

/**
 * How the net result of a position is made up, line by line (spec D5).
 *
 * The engine's identity is the authority and the screen never prints a breakdown that does not add up to it:
 *
 *   net = (value - invested) + fees collected + fees uncollected - gas
 *
 * `value - invested` is split in two for a lending or staking position (what the balance accrued by itself,
 * and what the price did); a pool has no accrual line, it earns through fees. Impermanent loss and the accrual
 * inside the value change are informational and never enter the sum. An unknown figure is `null`, never zero.
 * Every amount is a decimal string and is added exactly (`shared/lib/decimal`), so the lines sum to `net_usd` to the digit.
 */

export type PositionKind = CryptoPositionView['kind']

export type BreakdownLineId = 'accrual' | 'priceChange' | 'feesCollected' | 'feesUncollected' | 'fees' | 'gas'

/** Why a figure is absent; each is a key under `cryptoTrack.positions.reason.*`. */
export type MissingReason = 'noPrice' | 'noHistory' | 'notRead' | 'notApplicable'

export interface BreakdownLine {
  id: BreakdownLineId
  /** A signed decimal amount in USD, or `null` when it is not known. */
  amount: string | null
  /** The number is an approximation (the adapter said so); exact figures (gas, collected fees) stay without it. */
  approximate: boolean
  /** Set when `amount` is `null`. */
  reason?: MissingReason
}

/** The figure as a decimal string, or `null` when it is absent, empty or not a number. */
const known = (value: string | undefined | null): string | null =>
  value === undefined || value === null ? null : parseDecimal(value)

const isZero = (value: string): boolean => decimalSub(value, '0') === '0'

/** Nothing to show: the figure is absent or an exact zero. */
const isEmpty = (value: string | null): boolean => value === null || isZero(value)

/** A lending or a staking position accrues on its own balance; a pool does not. */
export const accruesOnBalance = (kind: PositionKind): boolean => kind !== 'pool'

/**
 * Whether the fee figures are a thing for this position: always for a pool, which earns through them, and for a
 * supply or a stake once something was claimed or is pending. The breakdown and the row figure follow the same
 * rule, so the table cannot print a dash for fees the detail lists.
 */
export function feesApply(position: CryptoPositionView): boolean {
  if (!accruesOnBalance(position.kind)) return true
  return !(isEmpty(known(position.fees_collected_usd)) && isEmpty(known(position.fees_uncollected_usd)))
}

/** The symbols of the assets that came without a USD value (no price source for the token). */
export function unpricedSymbols(position: CryptoPositionView): string[] {
  return position.assets.filter((asset) => asset.value_usd === undefined).map((asset) => asset.symbol.toUpperCase())
}

/** Why a figure that depends on a price is missing: a token without a price, else the position could not be read. */
export function priceReason(position: CryptoPositionView): MissingReason {
  return unpricedSymbols(position).length ? 'noPrice' : 'notRead'
}

export function positionBreakdown(position: CryptoPositionView): BreakdownLine[] {
  const value = known(position.value_usd)
  const invested = known(position.invested_usd)
  const accrued = known(position.accrued_usd)
  const collected = known(position.fees_collected_usd)
  const uncollected = known(position.fees_uncollected_usd)
  const gas = known(position.gas_usd)
  const accrues = accruesOnBalance(position.kind)
  const unknownReason = priceReason(position)
  const approximate = position.approximate

  const lines: BreakdownLine[] = []
  if (accrues) {
    lines.push(
      accrued === null
        ? { id: 'accrual', amount: null, approximate, reason: unknownReason }
        : { id: 'accrual', amount: accrued, approximate },
    )
  }

  // With an accrual line the price change is what is left of the value change; without the accrual we cannot split it.
  const canSplit = value !== null && invested !== null && (!accrues || accrued !== null)
  const priceChange = canSplit ? decimalSub(value, invested) : null
  const priceChangeNet =
    priceChange !== null && accrues && accrued !== null ? decimalSub(priceChange, accrued) : priceChange
  lines.push(
    priceChangeNet !== null
      ? { id: 'priceChange', amount: priceChangeNet, approximate }
      : {
          id: 'priceChange',
          amount: null,
          approximate,
          reason: value === null ? unknownReason : invested === null ? 'noHistory' : unknownReason,
        },
  )

  if (!feesApply(position)) {
    // Fees belong to a pool: for a supply or a stake with nothing claimed or pending it is "does not exist", not zero.
    lines.push({ id: 'fees', amount: null, approximate: false, reason: 'notApplicable' })
  } else {
    lines.push(
      collected === null
        ? { id: 'feesCollected', amount: null, approximate: false, reason: 'notRead' }
        : { id: 'feesCollected', amount: collected, approximate: false },
      uncollected === null
        ? { id: 'feesUncollected', amount: null, approximate, reason: 'notRead' }
        : { id: 'feesUncollected', amount: uncollected, approximate },
    )
  }

  lines.push(
    gas === null
      ? { id: 'gas', amount: null, approximate: false, reason: 'noHistory' }
      : { id: 'gas', amount: decimalSub('0', gas), approximate: false },
  )
  return lines
}

/** The exact sum of the lines that are known; equals `net_usd` when every line is. */
export function knownSum(lines: BreakdownLine[]): string {
  return lines.reduce((sum, line) => (line.amount === null ? sum : (decimalAdd(sum, line.amount) ?? sum)), '0')
}

/** Whether every line is known, so the sum is the whole story. */
export const isComplete = (lines: BreakdownLine[]): boolean =>
  lines.every((line) => line.amount !== null || line.reason === 'notApplicable')

/** What went back out of the position: what was ever put in minus the capital still inside. `null` if either is unknown. */
export function withdrawnUsd(position: CryptoPositionView): string | null {
  const putIn = known(position.put_in_usd)
  const invested = known(position.invested_usd)
  return putIn === null || invested === null ? null : decimalSub(putIn, invested)
}
