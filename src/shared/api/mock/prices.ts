/** Last known USD prices, mirroring what the Rust `price_cache` would hold. */
export const mockPrices: Record<string, number> = {
  BTC: 90000,
  ETH: 3000,
  USD: 1,
  USDT: 1,
  RUB: 0.0105,
  EUR: 1.08,
  THB: 0.028,
  /** The flat and the car (sold on Wallets-4 · 21), a unit each: «Имущество 35 580,70 $» on Money 2 · 12. */
  FLAT: 25580.7,
  CAR: 10000,
}

export const priceOf = (symbol: string) => mockPrices[symbol.toUpperCase()] ?? 0
