import { withoutLoyalty } from '@/shared/lib/store-tree'

/** The order the account groups appear in: money for every day first, money that is not yours last. */
export const PURPOSE_ORDER: Purpose[] = ['everyday', 'savings', 'investment', 'transit']

export interface PurposeGroup {
  purpose: Purpose
  nodes: StoreTreeNode[]
  /** Sum of the groups' accounts in the base currency, sub-accounts included. */
  total: number
}

/**
 * Top-level accounts by what the money is for: archived ones and bonus programmes are left out
 * (bonuses live under Operations), empty groups are not listed.
 */
export function groupByPurpose(roots: StoreTreeNode[]): PurposeGroup[] {
  const live = withoutLoyalty(roots).filter((node) => !node.store.archived)
  return PURPOSE_ORDER.map((purpose) => {
    const nodes = live.filter((node) => node.store.purpose === purpose)
    return { purpose, nodes, total: nodes.reduce((sum, node) => sum + Number(node.subtree_base), 0) }
  }).filter((group) => group.nodes.length > 0)
}
