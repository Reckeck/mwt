export interface CryptoOperationsProps {
  /** Accounts a wallet fixes (its own); empty reads every followed account and lets `?account=` narrow it. */
  accountIds?: string[]
}
