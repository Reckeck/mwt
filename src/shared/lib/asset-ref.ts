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
