export interface TransactionEditButtonProps {
  tx: Transaction
  /** Wallet the edit is opened from; defaults to the transaction's own. */
  storeId?: string
}
