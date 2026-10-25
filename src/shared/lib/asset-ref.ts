/** A fiat currency as the backend's `AssetRef` (codes upper-case, like Rust's `AssetRef::fiat`). */
export function fiatRef(code: string): AssetRef {
  const symbol = code.trim().toUpperCase()
  return { symbol, name: symbol, coingecko_id: null, chain: 'fiat', contract: null, quote: symbol }
}

/** A bare ticker with no network: matches the same ticker on any chain (pair sides). */
export function tickerRef(raw: string): AssetRef {
  const symbol = raw.trim().toUpperCase()
  return { symbol, name: symbol, coingecko_id: null, chain: null, contract: null, quote: symbol }
}

/**
 * The same identity as Rust's `AssetRef::key()`: what the ledger sums a
 * balance by. Two refs with one key are one asset; a ticker with a CoinGecko
 * id and the same ticker without one are two (as the backend sees them).
 */
export function assetKey(asset: AssetRef): string {
  if (asset.contract?.startsWith('perp:')) return `${asset.contract}:${asset.symbol}`
  if (asset.chain && asset.contract) return `token:${asset.chain}:${asset.symbol}:${asset.contract}`
  if (asset.coingecko_id) return `coin:${asset.coingecko_id}`
  return `${asset.chain === 'fiat' ? 'fiat' : 'asset'}:${asset.symbol}`
}

/** What the operations search matches an asset by: its ticker, from an `assetKey` (`token:eth:USDT:0x…` → `USDT`). */
export function assetSearchText(key: string): string {
  const parts = key.split(':')
  if (parts[0] === 'token') return parts[2] ?? key
  if (parts[0] === 'perp') return parts.at(-1) ?? key
  return parts.slice(1).join(':') || key
}
