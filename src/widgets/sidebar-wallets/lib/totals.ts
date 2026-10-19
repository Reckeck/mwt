import { formatMoney } from '@/shared/lib/format'

/** A parent shows its whole subtree, a leaf its own value, rounded like the wallet page; nothing for zero. */
export function nodeTotal(node: StoreTreeNode, currency: string): string {
  const value = node.children.length ? node.subtree_base : node.own_base
  if (!Number(value)) return ''
  return formatMoney(value, { currency, decimals: 2 })
}

/** What the live wallets of a kind hold together; nothing for zero. */
export function groupTotal(nodes: StoreTreeNode[], currency: string): string {
  const sum = nodes.filter((node) => !node.store.archived).reduce((acc, node) => acc + Number(node.subtree_base), 0)
  if (!sum) return ''
  return formatMoney(sum, { currency, decimals: 2 })
}

/** A kind takes another wallet unless it is capped (cash) and the cap is reached. */
export function canAddOfKind(info: WalletKindInfo, count: number): boolean {
  return info.user_creatable && (info.max_roots === null || count < info.max_roots)
}
