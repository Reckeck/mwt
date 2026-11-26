import { decimalCmp, decimalShare, decimalSub } from '@/shared/lib/decimal'

const RATE_PLACES = 8
const MONEY_PLACES = 2
const PERCENT_PLACES = 1

const isPositive = (value: string | null): value is string => value !== null && decimalCmp(value, '0') === 1

/**
 * An exchange as the notice of board 462813 tells it, all in the given currency: the deal rate (how
 * much was given for one got), the market rate, and what the deal lost on the rate (negative: it
 * gained) with its share of the market value in percent.
 */
export interface ExchangeQuote {
  rate: string
  market: string | null
  loss: string | null
  percent: string | null
}

/** The quote of a deal; `market` is what the given amount fetches at the market rate. `null` until both amounts are positive. */
export function exchangeQuote(gave: string, got: string, market: string | null): ExchangeQuote | null {
  if (!isPositive(gave) || !isPositive(got)) return null
  const rate = decimalShare(gave, '1', got, RATE_PLACES)
  if (rate === null) return null
  const short = isPositive(market) ? decimalSub(market, got) : null
  if (!isPositive(market) || short === null) return { rate, market: null, loss: null, percent: null }
  return {
    rate,
    market: decimalShare(gave, '1', market, RATE_PLACES),
    loss: decimalShare(gave, short, market, MONEY_PLACES),
    percent: decimalShare(short, '100', market, PERCENT_PLACES),
  }
}
