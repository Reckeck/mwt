export interface TransactionRowProps {
  tx: Transaction
  /** Resolved store name; falls back to the raw id when unknown. */
  storeName: string
}
