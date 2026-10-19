export interface StoreRowLinkProps {
  store: Store
  /** The value shown on the right, in `currency`. */
  amount: string
  currency: string
  /** Nesting under the card's wallet: 1 is a direct sub-account. */
  depth?: number
  /** Share of the parent, 0..1; omitted, no share is shown. */
  share?: number | null
  decimals?: number
}
