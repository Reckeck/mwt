import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'

export const WALLET_CATALOG_QUERY_KEY = ['wallet-catalog'] as const

/** What every wallet kind allows; static domain data from Rust, fetched once. */
export function useWalletCatalog() {
  return useQuery({
    queryKey: WALLET_CATALOG_QUERY_KEY,
    queryFn: () => call<WalletCatalog>('get_wallet_catalog'),
    staleTime: Number.POSITIVE_INFINITY,
  })
}

export function kindInfo(catalog: WalletCatalog | undefined, kind: StoreKind): WalletKindInfo | undefined {
  return catalog?.kinds.find((info) => info.kind === kind)
}

/** Short, human label of a binding: «USD», «BTC ↔ USDT», «Futures». */
export function bindingLabel(binding: Binding): string {
  if (binding === 'any') return ''
  if ('asset' in binding) return binding.asset.symbol.toUpperCase()
  if ('pair' in binding) return `${binding.pair.base.symbol.toUpperCase()} ↔ ${binding.pair.quote.symbol.toUpperCase()}`
  return binding.market.charAt(0).toUpperCase() + binding.market.slice(1)
}

/** Quote assets a pair symbol may end with, longest first so USDT wins over USD. */
const PAIR_QUOTES = ['FDUSD', 'USDT', 'USDC', 'BUSD', 'TUSD', 'DAI', 'USD', 'EUR', 'TRY', 'BTC', 'ETH', 'BNB']

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
