/**
 * Wallets form a tree of at most three levels (wallet → sub-account →
 * sub-account). The backend owns it: `get_store_tree` returns it already
 * nested, sorted and totalled, so these helpers only reshape that answer.
 */

/**
 * Query key of `get_store_tree`. It sits under `['stores']`, so every mutation
 * that already invalidates the store list refreshes the tree too.
 */
export const STORE_TREE_QUERY_KEY = ['stores', 'tree'] as const

/** Depth-first flat order: parent, then its whole subtree. */
export function flattenStoreTree(roots: StoreTreeNode[]): StoreTreeNode[] {
  return roots.flatMap((node) => [node, ...flattenStoreTree(node.children)])
}

/** Ids from the top-level wallet down to `id`'s parent; empty for a root or an unknown id. */
export function storeAncestorIds(roots: StoreTreeNode[], id: string): string[] {
  for (const node of roots) {
    if (node.store.id === id) return []
    const below = storeAncestorIds(node.children, id)
    if (below.length || node.children.some((child) => child.store.id === id)) return [node.store.id, ...below]
  }
  return []
}

/** Every node below `node`, without `node` itself. */
export function storeDescendants(node: StoreTreeNode): StoreTreeNode[] {
  return flattenStoreTree(node.children)
}

/**
 * The tree without bonus programmes: they are counted in totals and scopes but
 * live under Operations → Bonuses, not among wallets (plan G1).
 */
export function withoutLoyalty(roots: StoreTreeNode[]): StoreTreeNode[] {
  return roots
    .filter((node) => node.store.kind !== 'loyalty')
    .map((node) => (node.children.length ? { ...node, children: withoutLoyalty(node.children) } : node))
}
