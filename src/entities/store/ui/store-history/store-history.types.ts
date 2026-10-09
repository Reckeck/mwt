export interface StoreHistoryProps {
  storeId: string
  /** Also list the transactions of every sub-account. Default on. */
  includeChildren?: boolean
}
