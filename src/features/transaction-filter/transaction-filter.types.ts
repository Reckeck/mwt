export interface TransactionFilterBarProps {
  /** The page fixes the wallet (a wallet's own history): no wallet picker. */
  fixedScope?: boolean
  /** Prefix for field ids, so two bars on one screen do not clash. */
  idPrefix?: string
}
