export interface ImportSettingsFormProps {
  /** The exchange wallet (root store). */
  storeId: string
}

/** How one exchange account is booked: as before, into a chosen wallet, or not at all. */
export type ImportRouteMode = 'default' | 'store' | 'skip'
