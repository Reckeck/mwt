import type { UiAction } from '@/shared/ui/context-menu'

export interface CryptoTxListProps {
  rows: CryptoOpRow[]
  /** How many operations match in all; the rest is fetched by `Show more`. */
  total: number
  loadingMore?: boolean
  /** The menu of a row («⋯»): the list only draws it, what it does is the feature's. */
  rowActions?: (row: CryptoOpRow) => UiAction[]
}
