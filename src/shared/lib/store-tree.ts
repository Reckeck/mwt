/**
 * Wallets form a free tree over `Store.parent_id` (sub-accounts, per-currency
 * sub-wallets). The backend validates it — no cycles, no deleting a parent —
 * and these helpers only shape the flat `list_stores` result for display.
 */

export interface StoreNode {
  store: Store
  children: StoreNode[]
  /** 0 for a top-level wallet. */
  depth: number
}

/** Live wallets first, archived last; alphabetical inside each group. */
function byStatusThenName(left: StoreNode, right: StoreNode): number {
  if (left.store.archived !== right.store.archived) return left.store.archived ? 1 : -1
  return left.store.name.localeCompare(right.store.name, undefined, { sensitivity: 'base' })
}

/** Build the tree; a wallet whose parent is unknown becomes a root rather than vanishing. */
export function buildStoreTree(stores: Store[]): StoreNode[] {
  const nodes = new Map<string, StoreNode>()
  for (const store of stores) nodes.set(store.id, { store, children: [], depth: 0 })
  const roots: StoreNode[] = []
  for (const node of nodes.values()) {
    const parent = node.store.parent_id ? nodes.get(node.store.parent_id) : undefined
    if (parent) parent.children.push(node)
    else roots.push(node)
  }
  const settle = (list: StoreNode[], depth: number) => {
    list.sort(byStatusThenName)
    for (const node of list) {
      node.depth = depth
      settle(node.children, depth + 1)
    }
  }
  settle(roots, 0)
  return roots
}

/** Depth-first flat order: parent, then its whole subtree. */
export function flattenStoreTree(roots: StoreNode[]): StoreNode[] {
  return roots.flatMap((node) => [node, ...flattenStoreTree(node.children)])
}

/** Ids of `id` and everything below it — none of them can become its parent. */
export function storeSubtreeIds(stores: Store[], id: string): Set<string> {
  const out = new Set([id])
  let grew = true
  while (grew) {
    grew = false
    for (const store of stores) {
      if (store.parent_id && out.has(store.parent_id) && !out.has(store.id)) {
        out.add(store.id)
        grew = true
      }
    }
  }
  return out
}

/** Parent chain from the top-level wallet down to `id`'s parent. */
export function storeAncestors(stores: Store[], id: string): Store[] {
  const byId = new Map(stores.map((store) => [store.id, store]))
  const chain: Store[] = []
  const seen = new Set<string>()
  let parentId = byId.get(id)?.parent_id ?? null
  while (parentId && !seen.has(parentId)) {
    seen.add(parentId)
    const parent = byId.get(parentId)
    if (!parent) break
    chain.unshift(parent)
    parentId = parent.parent_id
  }
  return chain
}
