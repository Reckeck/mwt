import { purposeGroup } from '@/entities/store'

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
  return PURPOSE_ORDER.map((purpose) => ({ purpose, ...purposeGroup(roots, purpose) })).filter(
    (group) => group.nodes.length > 0,
  )
}
