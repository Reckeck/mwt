export interface TransactionRowProps {
  tx: Transaction
  /** Wallet whose history this is: only its side of the amounts is shown. */
  storeId?: string | null
  /** Fee and spread of this transaction, when it has any. */
  loss?: TxLoss | null
  /** The row opens the details panel: its body becomes a button. */
  openable?: boolean
}
