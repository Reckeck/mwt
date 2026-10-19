import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { i18n } from '@/shared/config/i18n'

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

/** Short, human label of a binding: «USD», «BTC ↔ USDT», «Фьючерсы». */
export function bindingLabel(binding: Binding): string {
  if (binding === 'any') return ''
  if ('asset' in binding) return binding.asset.symbol.toUpperCase()
  if ('pair' in binding) return `${binding.pair.base.symbol.toUpperCase()} ↔ ${binding.pair.quote.symbol.toUpperCase()}`
  return i18n.global.t(`store.market.${binding.market}`)
}

/**
 * The binding badge next to a wallet's name, or `''` when the name already
 * says it («Futures» needs no «Фьючерсы» badge).
 */
export function bindingBadge(store: Pick<Store, 'name' | 'binding'>): string {
  const label = bindingLabel(store.binding)
  if (!label) return ''
  const name = store.name.toLowerCase()
  const raw = typeof store.binding === 'object' && 'market' in store.binding ? store.binding.market : ''
  return name.includes(label.toLowerCase()) || (raw && name.includes(raw)) ? '' : label
}

/** Kept here for the callers that read it from the store entity. */
export { PAIR_QUOTES, parsePairSymbol } from '@/shared/lib/pair'
