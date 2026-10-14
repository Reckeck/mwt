import type { Component } from 'vue'

import { formatMoney } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'

/** A wallet as a row of a picker or the search: the name, and what tells it apart under it. */
export interface WalletRow {
  id: string
  title: string
  /** Parent path (or the wallet kind for a top-level one), bound asset and balance, joined by «·». */
  detail: string
  icon: Component
  color: string | undefined
  /** Lower-case text a search looks in. */
  haystack: string
}

/** One wallet of the store tree as a two-line row, the same in «To» of an operation and in the search. */
export function walletRow(node: StoreTreeNode, t: (key: string) => string, baseCurrency: string): WalletRow {
  const parent = node.path.slice(0, -1).join(' › ')
  const binding =
    typeof node.store.binding === 'object' && 'asset' in node.store.binding ? node.store.binding.asset.symbol : ''
  const balance = formatMoney(node.subtree_base, { currency: baseCurrency })
  return {
    id: node.store.id,
    title: node.store.name,
    detail: [parent || t(`stores.kind.${node.store.kind}`), binding, balance].filter(Boolean).join(' · '),
    icon: storeIcon(node.store),
    color: node.store.color || undefined,
    haystack: `${node.path.join(' ')} ${binding}`.toLowerCase(),
  }
}
