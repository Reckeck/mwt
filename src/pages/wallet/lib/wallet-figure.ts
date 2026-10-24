/** The sum of a wallet's header and what its line under it says. */
export interface WalletFigure {
  amount: string
  /** Lowercase code of `amount`. */
  currency: string
  /** Set when the sum is in the wallet's own currency: the base sum and one base unit in it. */
  base: { amount: string; rate: number } | null
  /** «BTC, USDT»: what a wallet of several currencies holds, biggest first. */
  assets: string[]
}

/**
 * A wallet of one fiat currency other than the base shows that currency, as
 * its bank does (Currency-1 · 3); anything else shows the base sum and the
 * currencies it holds (Currency-1 · 4).
 */
export function walletFigure(positions: StorePosition[], total: string, base: string): WalletFigure {
  const held = positions.filter((position) => Number(position.quantity) !== 0)
  const assets = [...held]
    .sort((left, right) => Number(right.value_base) - Number(left.value_base))
    .map((position) => position.asset.symbol.toUpperCase())
  const only = held.length === 1 ? held[0] : null
  if (only && only.asset.chain === 'fiat' && only.asset.symbol.toLowerCase() !== base) {
    const price = Number(only.price_base)
    return {
      amount: only.quantity,
      currency: only.asset.symbol.toLowerCase(),
      base: { amount: only.value_base, rate: price ? 1 / price : 0 },
      assets,
    }
  }
  return { amount: total, currency: base, base: null, assets }
}
