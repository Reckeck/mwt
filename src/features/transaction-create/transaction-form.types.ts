export interface TransactionFormProps {
  /** Wallet the form is opened on; it fills the store fields. */
  storeId: string
  /** A stored transaction to edit; `null` creates a new one. */
  transaction?: Transaction | null
  /** Calculator plan to prefill once; a new object applies again. */
  plan?: TransactionPlan | null
  disabled?: boolean
  /** Opens a transfer to this side (a contact's card: «write a transfer here»). */
  counterpart?: AccountRef | null
}

/** A calculator result handed to the form. */
export interface TransactionPlan {
  mode: CalcMode
  from: AssetRef | null
  to: AssetRef | null
  amount: string
  /** USD price of `from`. */
  price: string
}

export interface TransactionCreateDialogProps {
  storeId: string
  /** Archived wallets take no new transactions. */
  disabled?: boolean
  /** Calculator plan to prefill. */
  plan?: TransactionPlan | null
  /** Opens a transfer to this side. */
  counterpart?: AccountRef | null
}

export interface TransactionEditDialogProps {
  transaction: Transaction
  /** Wallet the edit was opened from; defaults to the transaction's own. */
  storeId?: string
}
