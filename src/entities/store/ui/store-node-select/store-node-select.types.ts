export interface StoreNodeSelectProps {
  /** Nodes to offer, flat or nested; each shows as its full path. */
  nodes: StoreTreeNode[]
  /** Label of the `''` option (e.g. "All wallets"); without it there is no such option. */
  emptyLabel?: string
}
