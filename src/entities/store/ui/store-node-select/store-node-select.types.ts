export interface StoreNodeSelectProps {
  /** Nodes to offer, flat or nested; each shows as its full path. */
  nodes: StoreTreeNode[]
  /** Label of the `''` option (e.g. "All wallets"); without it there is no such option. */
  emptyLabel?: string
  /** Hint shown while nothing is picked, not choosable itself («Выберите счёт»). */
  placeholder?: string
  /** Offer exactly `nodes`, not their sub-accounts too (for a filtered list). */
  flat?: boolean
  /** Several nodes at once (a filter), bound with `v-model:ids`; none picked = `emptyLabel`. */
  multiple?: boolean
  /** Offer only nodes with this purpose (the emergency fund takes savings accounts only). */
  purpose?: Purpose
  /** Marks the field as wrong (red border), next to an error under it. */
  invalid?: boolean
}
