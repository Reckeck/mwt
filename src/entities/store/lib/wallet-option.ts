import type { Component } from 'vue'

import { formatMoney } from '@/shared/lib/format'
import { storeIcon } from '@/shared/lib/store-icons'

/** A wallet as a row of a picker or the search: the name, and what tells it apart under it. */
export interface WalletRow {
  id: string
  title: string
  /** Parent path (or the wallet kind for a top-level one), bound asset and balance, joined by «·». */
  detail: string
  /** The path joined by «·» («Тинькофф · Карта»): the name on the wallet plate and a sheet row (boards 533927, 532847). */
  fullName: string
  /** The bound asset, the second line of a sheet row; empty for an unbound wallet. */
  subtitle: string
  /** The balance in the wallet's own currency when it holds one, else in the base currency. */
  balance: string
  /** `≈ 220 700 ₽` under a balance in another currency than the base one (board 984555); empty otherwise. */
  baseHint: string
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
  // money: plain — a part of a picker option's and a search row's text, joined with the path and the asset
  const baseBalance = formatMoney(node.subtree_base, { currency: baseCurrency })
  const only = node.balances.length === 1 ? node.balances[0] : undefined
  // money: plain — the right-hand text of a sheet row and of the wallet plate
  const balance = only ? formatMoney(only.quantity, { currency: only.asset.symbol.toLowerCase() }) : baseBalance
  return {
    id: node.store.id,
    title: node.store.name,
    detail: [parent || t(`stores.kind.${node.store.kind}`), binding, baseBalance].filter(Boolean).join(' · '),
    fullName: node.path.join(' · '),
    subtitle: binding,
    balance,
    baseHint: only && only.asset.symbol.toLowerCase() !== baseCurrency.toLowerCase() ? `≈ ${baseBalance}` : '',
    icon: storeIcon(node.store),
    color: node.store.color || undefined,
    haystack: `${node.path.join(' ')} ${binding}`.toLowerCase(),
  }
}
