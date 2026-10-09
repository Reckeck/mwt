export interface AccountRefPickerProps {
  /** Offer the user's own wallets. */
  stores?: boolean
  /** Offer saved contacts and their accounts. */
  contacts?: boolean
  /** Offer free text («someone at the market»). */
  raw?: boolean
  /** A wallet that cannot be picked (the other end of a transfer). */
  excludeStoreId?: string | null
  /** Label of the empty choice; without it a side is required. */
  emptyLabel?: string
}
