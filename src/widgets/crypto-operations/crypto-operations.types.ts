export interface CryptoOperationsProps {
  /** Accounts a wallet fixes (its own); empty reads every followed account and lets `?account=` narrow it. */
  accountIds?: string[]
  /** No page chrome of its own: inside a wallet's tab. */
  bare?: boolean
}
