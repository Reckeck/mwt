export interface AddTransactionFormProps {
  storeId: string
  disabled?: boolean
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
