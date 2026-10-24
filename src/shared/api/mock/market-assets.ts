/** Asset search and terminal candles. */

export function mockSearch(rawQuery: string): AssetSearchHit[] {
  const known = [
    { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin' },
    { symbol: 'ETH', name: 'Ethereum', coingecko_id: 'ethereum' },
    { symbol: 'SOL', name: 'Solana', coingecko_id: 'solana' },
    { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether' },
  ]
  const needle = (rawQuery ?? '').toLowerCase()
  return known
    .filter((entry) => entry.symbol.toLowerCase().includes(needle) || entry.name.toLowerCase().includes(needle))
    .map((entry) => ({ ...entry, chain: 'ethereum', contract: null, rank: 1n }))
}

export function mockCandles(input: GetCandlesInput): GetCandlesOutput {
  const limit = Math.min(Math.max(input.limit || 300, 1), 1000)
  const stepMs = (input.interval_min || 60) * 60_000
  const now = Math.floor(Date.now() / stepMs) * stepMs
  const basePrice = 60000 + ((input.symbol.length * 1500) % 30000)
  let price = basePrice
  const candles: CandlePoint[] = []
  for (let step = limit - 1; step >= 0; step--) {
    const drift = Math.sin(step / 17) * 0.004 + Math.cos(step / 6) * 0.002
    const open = price
    const close = open * (1 + drift)
    const high = Math.max(open, close) * (1 + 0.002 * ((step * 7) % 5))
    const low = Math.min(open, close) * (1 - 0.002 * ((step * 3) % 5))
    const volume = String(50 + ((step * 37) % 200))
    candles.push({
      ts: new Date(now - step * stepMs).toISOString(),
      open: open.toFixed(2),
      high: high.toFixed(2),
      low: low.toFixed(2),
      close: close.toFixed(2),
      volume,
    })
    price = close
  }
  return { candles }
}
