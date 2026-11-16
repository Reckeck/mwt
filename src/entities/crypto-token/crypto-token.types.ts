/** One token of an address that waits for the user's decision, ready to show. */
export interface QuarantineRow {
  id: string
  chain: Chain
  contract: string
  /** Untrusted text from a chain, already cleaned by the backend; shown as plain text only. */
  symbol: string
  name: string
  quantity: string
  /** RFC3339 of the first operation with it. */
  firstSeen: string
  warnings: TokenWarning[]
}
