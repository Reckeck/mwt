export interface TransactionFilterBarProps {
  /** The page fixes the wallet (a wallet's own history): no wallet picker. */
  fixedScope?: boolean
  /** Prefix for field ids, so two bars on one screen do not clash. */
  idPrefix?: string
}

export interface FilterFieldProps {
  /** Prefix for the field's ids, as the bar's. */
  idPrefix: string
  /** In the row, without a label: the field names itself to a screen reader. */
  bare?: boolean
}
