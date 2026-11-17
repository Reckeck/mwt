export interface StoreTransactionsProps {
  storeId: string
  /** Also list the transactions of every sub-account. Default on. */
  includeChildren?: boolean
  /**
   * Just the rows (Wallets-1 · 5): no title and no filters, a link to the full
   * list under them. The wallet page's operations tab.
   */
  bare?: boolean
  /** The base currency when the page already has it: the day totals show its mark from the first frame, not after the summary. */
  baseCurrency?: string
}
