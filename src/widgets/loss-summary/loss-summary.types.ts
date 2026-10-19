export interface LossSummaryProps {
  /** A report to show; without one the widget fetches the last twelve months. */
  report?: LossReport | null
  /** Only these wallets (and their sub-accounts); the «by wallet» group then goes. */
  storeIds?: string[]
  /** Draw nothing when there were no losses (a wallet page, not the analytics). */
  hideEmpty?: boolean
  /** A heading drawn with the figures, so a hidden widget leaves no stray title. */
  title?: string
}
