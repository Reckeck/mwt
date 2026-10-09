export interface StoreCardProps {
  /** A top-level wallet with its whole subtree. */
  node: StoreTreeNode
  /** Currency of the `*_base` totals. */
  baseCurrency: string
}
