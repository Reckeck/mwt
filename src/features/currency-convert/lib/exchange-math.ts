import { decimalCmp, decimalShare, decimalSub } from '@/shared/lib/decimal'

const RATE_PLACES = 8

const isPositive = (value: string) => decimalCmp(value, '0') === 1

/** The deal rate: how much is got for one given; `null` until both amounts are positive. */
export function exchangeRate(gave: string, got: string): string | null {
  if (!isPositive(gave) || !isPositive(got)) return null
  return decimalShare(got, '1', gave, RATE_PLACES)
}

/** What the deal cost over the market: the market value of the given amount minus what was got (negative: better than market). */
export function exchangeLoss(market: string | null, got: string): string | null {
  if (market === null || !isPositive(got)) return null
  return decimalSub(market, got)
}
