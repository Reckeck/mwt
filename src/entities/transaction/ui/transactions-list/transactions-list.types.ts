export interface TransactionsListProps {
  transactions: Transaction[]
  /** Wallet whose history this is: rows show only its side of the amounts. */
  storeId?: string | null
}
