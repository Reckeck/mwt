import type { UiAction } from '@/shared/ui/context-menu'

export interface TransactionsListProps {
  transactions: Transaction[]
  /** Wallet whose history this is: rows show only its side of the amounts. */
  storeId?: string | null
  /** Losses by transaction id, for the loss column. */
  losses?: Record<string, TxLoss> | null
  /** Rows open the details panel (`open` event). */
  openable?: boolean
  /** Right-click menu of a row; the page decides what can be done with it. */
  rowActions?: ((tx: Transaction) => UiAction[]) | null
  /** Rows under a sticky header per day with the day's net in the base currency. */
  groupByDay?: boolean
  /** Base units per USD and the base code, for the day totals. */
  fxPerUsd?: string | null
  baseCurrency?: string
  /**
   * The page loads the list in pages from the backend: how many match in all.
   * «Показать ещё» then asks for the next page (`more`) instead of revealing rows.
   */
  total?: number | null
  /** The next page is on its way. */
  loadingMore?: boolean
}
