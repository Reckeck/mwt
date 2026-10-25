export interface AdjustmentDialogProps {
  /** The account whose balances are corrected. */
  storeId: string
  /** A stored correction to edit; `null` makes a new one. */
  transaction?: Transaction | null
}

/** One asset row of the correction screen. */
export interface AdjustmentRow {
  asset: AssetRef
  /** The ledger balance at the correction's moment. */
  before: string
  /** What is really there; empty = unchanged. */
  actual: string
}

/** What the app holds in the account: the sum in its own asset and, for another currency, the base sum. */
export interface AdjustmentInApp {
  own: { amount: string; currency: string }
  base?: { amount: string; currency: string }
}
