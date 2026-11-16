import { decimalAdd, decimalSub, parseDecimal } from '@/shared/lib/decimal'

/** Totals of the positions on the screen's summary tiles. A figure nobody knows is left out, never counted as 0. */

export interface FigureTotal {
  /** An exact decimal string; `null` when no position had the figure. */
  sum: string | null
  /** How many positions could not give it (shown as the tile's reason). */
  missing: number
}

const toDecimal = (value: string | undefined): string | null => (value === undefined ? null : parseDecimal(value))

export function sumOf(
  positions: CryptoPositionView[],
  pick: (position: CryptoPositionView) => string | undefined,
): FigureTotal {
  let sum: string | null = null
  let missing = 0
  for (const position of positions) {
    const figure = toDecimal(pick(position))
    if (figure === null) missing += 1
    else sum = sum === null ? figure : (decimalAdd(sum, figure) ?? sum)
  }
  return { sum, missing }
}

/** The net result as a percent of what was put in, over the positions that have both (a zero put-in has no percent). */
export function netPercent(positions: CryptoPositionView[]): number | null {
  const both = positions.filter((position) => {
    const putIn = toDecimal(position.put_in_usd)
    return toDecimal(position.net_usd) !== null && putIn !== null && decimalSub(putIn, '0') !== '0'
  })
  const putIn = sumOf(both, (position) => position.put_in_usd).sum
  const net = sumOf(both, (position) => position.net_usd).sum
  // The sums are exact; only the final ratio, a percent for display, leaves the decimal domain.
  if (putIn === null || net === null || decimalSub(putIn, '0') === '0') return null
  return (Number(net) / Number(putIn)) * 100
}

export const ALL_PROTOCOLS = 'all'

/** The protocols present in the list, in a stable order, for the filter chips. */
export function protocolsOf(positions: CryptoPositionView[]): string[] {
  return [...new Set(positions.map((position) => position.protocol.split('-')[0]))].sort()
}

export function matchesProtocol(position: CryptoPositionView, protocol: string): boolean {
  return protocol === ALL_PROTOCOLS || position.protocol.split('-')[0] === protocol
}
