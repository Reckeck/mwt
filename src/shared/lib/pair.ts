/** Trading pairs as tickers: `BTCUSDT`, `ETH/USDC`… One place for every pair field. */

/** Quote assets a pair symbol may end with, longest first so USDT wins over USD. */
export const PAIR_QUOTES = ['FDUSD', 'USDT', 'USDC', 'BUSD', 'TUSD', 'DAI', 'USD', 'EUR', 'TRY', 'BTC', 'ETH', 'BNB']

/** «BTCUSDT», «btc/usdt», «ETH-USDC» → base and quote tickers, or `null`. */
export function parsePairSymbol(raw: string): { base: string; quote: string } | null {
  const text = raw.trim().toUpperCase()
  const split = text.split(/[/\-_: ]+/).filter(Boolean)
  if (split.length === 2 && split[0] !== split[1]) return { base: split[0], quote: split[1] }
  if (split.length !== 1) return null
  for (const quote of PAIR_QUOTES) {
    if (text.length > quote.length && text.endsWith(quote)) {
      return { base: text.slice(0, -quote.length), quote }
    }
  }
  return null
}

/** Watchlist entries are pairs against USDT; the chart takes the base ticker. */
export const baseOf = (pair: string) => pair.toUpperCase().replace(/USDT$/, '')

/** `btc`, `BTC/USDT`, `btc-usdt` → `BTCUSDT`; empty when nothing is left. */
export const pairOf = (raw: string) => {
  const base = raw
    .trim()
    .toUpperCase()
    .replace(/[/\-_ ]/g, '')
    .replace(/USDT?$/, '')
  return base ? `${base}USDT` : ''
}
