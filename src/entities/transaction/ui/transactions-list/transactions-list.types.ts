export interface TransactionsListProps {
  transactions: Transaction[]
  /** Wallet whose history this is: rows show only its side of the amounts. */
  storeId?: string | null
  /** Losses by transaction id, for the loss column. */
  losses?: Record<string, TxLoss> | null
  /** Rows open the details panel (`open` event). */
  openable?: boolean
}
