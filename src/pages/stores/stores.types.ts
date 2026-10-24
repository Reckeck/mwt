/** How the wallets are ordered: the biggest first, or by name. */
export type WalletSort = 'value' | 'name'

/** Which currency the total and the last column are in, or the per-currency breakdown. */
export type WalletMode = 'base' | 'second' | 'assets'

/** What a row asks the page to do. */
export interface WalletRowEvents {
  toggleArchive: [store: Store]
  delete: [store: Store]
}
