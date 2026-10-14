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
