import { flattenStoreTree } from '@/shared/lib/store-tree'

/** A taken name gets the next free number: the default must not collide with a sibling. */
export function uniqueName(base: string, tree: StoreTreeNode[], parentId: string | null | undefined) {
  const taken = new Set(
    flattenStoreTree(tree)
      .filter((node) => node.store.parent_id === parentId)
      .map((node) => node.store.name.trim().toLowerCase()),
  )
  if (!taken.has(base.toLowerCase())) return base
  let index = 2
  while (taken.has(`${base} ${index}`.toLowerCase())) index++
  return `${base} ${index}`
}
