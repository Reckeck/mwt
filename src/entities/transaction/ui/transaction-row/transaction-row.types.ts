export interface TransactionRowProps {
  tx: Transaction
  /** Wallet whose history this is: only its side of the amounts is shown. */
  storeId?: string | null
}
