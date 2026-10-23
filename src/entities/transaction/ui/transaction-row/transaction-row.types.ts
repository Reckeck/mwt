export interface TransactionRowProps {
  tx: Transaction
  /** Wallet whose history this is: only its side of the amounts is shown. */
  storeId?: string | null
  /** Fee and spread of this transaction, when it has any. */
  loss?: TxLoss | null
  /** The row opens the details panel: its body becomes a button. */
  openable?: boolean
  /** The row names its date and time: off under a day header, which already does. */
  dated?: boolean
  /**
   * Picking rows on a phone (Ops-1 · 4): a round mark takes the kind's place,
   * `true` filled. `null` — not picking.
   */
  selected?: boolean | null
}
