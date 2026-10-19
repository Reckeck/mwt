import { call } from '@/shared/api/ipc'
import { tickerRef } from '@/shared/lib/asset-ref'

/** A search hit as the asset a form stores. */
export function hitToRef(hit: AssetSearchHit): AssetRef {
  const symbol = hit.symbol.toUpperCase()
  return {
    symbol,
    name: hit.name || symbol,
    coingecko_id: hit.coingecko_id,
    chain: hit.chain,
    contract: hit.contract,
    quote: symbol,
  }
}

/**
 * The listed asset behind a bare ticker (a chart, a favourite), so it gets a
 * price feed; offline, the bare ticker still names it.
 */
export async function resolveTicker(symbol: string): Promise<AssetRef> {
  try {
    const hits = await call<AssetSearchHit[]>('search_assets', { query: symbol })
    const hit = hits.find((entry) => entry.symbol.toUpperCase() === symbol.toUpperCase())
    if (hit) return hitToRef(hit)
  } catch {
    // Offline search: fall through to the bare ticker.
  }
  return tickerRef(symbol)
}
