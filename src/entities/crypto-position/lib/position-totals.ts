/** What the open positions are worth together, and how many of them have no known worth (a dash, never a zero). */
export interface PositionTotals {
  /** USD, the open positions whose worth is known. */
  value: number
  /** Open positions left out of `value` because nobody knows their worth. */
  missing: number
}

export function positionTotals(positions: CryptoPositionView[]): PositionTotals {
  return positions
    .filter((position) => position.status === 'open')
    .reduce<PositionTotals>(
      (totals, position) =>
        position.value_usd === undefined || position.value_usd === null
          ? { ...totals, missing: totals.missing + 1 }
          : { ...totals, value: totals.value + Number(position.value_usd) },
      { value: 0, missing: 0 },
    )
}

/** The protocols that appear in the list, once each and in order of appearance. */
export const protocolsOf = (positions: CryptoPositionView[]): string[] => [
  ...new Set(positions.map((position) => position.protocol)),
]
