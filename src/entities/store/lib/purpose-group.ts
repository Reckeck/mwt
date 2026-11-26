import { withoutLoyalty } from '@/shared/lib/store-tree'

/** The one rule for the accounts that count toward capital: no bonus programmes, nothing archived. */
export function liveRoots(roots: StoreTreeNode[]): StoreTreeNode[] {
  return withoutLoyalty(roots).filter((node) => !node.store.archived)
}

/** The live top-level accounts kept for `purpose` and their sum in the base currency, sub-accounts included. */
export function purposeGroup(roots: StoreTreeNode[], purpose: Purpose): { nodes: StoreTreeNode[]; total: number } {
  const nodes = liveRoots(roots).filter((node) => node.store.purpose === purpose)
  return { nodes, total: nodes.reduce((sum, node) => sum + Number(node.subtree_base), 0) }
}
