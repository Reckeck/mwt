export interface TransactionsListProps {
  transactions: Transaction[]
  /** Resolve a store id to its display name; unknown ids render as-is. */
  storeName: (storeId: string) => string
}
