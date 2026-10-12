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
  /** `id` of the trigger, for an outer `<Label for>`. */
  id?: string
}

/** One row of the list: a title on top, the details under it. */
export interface AccountOption {
  /** `store:<id>`, `contact:<id>`, `contact:<id>:<account>`, `raw` or `''`. */
  value: string
  title: string
  detail: string
  /** Heading the row sits under. */
  group: string
  /** Lower-case text the search looks in. */
  haystack: string
  /** A contact's account: the contact row it sits under (`contact:<id>`). */
  parent?: string
  /** What kind of account this is, for the icon. */
  detailKind?: DetailKind
}
