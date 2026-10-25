import type { SumPart } from '@/shared/lib/money-parts'

/** A total for `<Money v-bind>`; the template shows it only when `amount` is not zero. */
type Total = Extract<SumPart, { amount: unknown }>

/** A parent shows its whole subtree, a leaf its own value, rounded like the wallet page; nothing for zero. */
export function nodeTotal(node: StoreTreeNode, currency: string): Total {
  const amount = node.children.length ? node.subtree_base : node.own_base
  return { amount, currency, decimals: 2 }
}

/** What the live wallets of a kind hold together; nothing for zero. */
export function groupTotal(nodes: StoreTreeNode[], currency: string): Total {
  const amount = nodes.filter((node) => !node.store.archived).reduce((acc, node) => acc + Number(node.subtree_base), 0)
  return { amount, currency, decimals: 2 }
}

/** A kind takes another wallet unless it is capped (cash) and the cap is reached. */
export function canAddOfKind(info: WalletKindInfo, count: number): boolean {
  return info.user_creatable && (info.max_roots === null || count < info.max_roots)
}
