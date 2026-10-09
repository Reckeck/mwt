export interface AddTransactionFormProps {
  storeId: string
  disabled?: boolean
  /** Calculator plan to prefill once; a new object applies again. */
  plan?: TransactionPlan | null
}

/** A calculator result handed to the form through `prefill`. */
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
}
