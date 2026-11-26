import { flattenStoreTree } from '@/shared/lib/store-tree'

/** An operation is written on a sub-account, never on a group that has live ones (`Main bank` holds `Card`). */
export function quickWalletNodes(tree: StoreTreeNode[] | undefined): StoreTreeNode[] {
  return flattenStoreTree(tree ?? []).filter(
    (node) => !node.store.archived && node.children.every((child) => child.store.archived),
  )
}
