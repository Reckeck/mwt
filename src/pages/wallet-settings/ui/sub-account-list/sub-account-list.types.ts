export interface SubAccountListProps {
  children: StoreChildRow[]
  /** The wallet's base currency: sub-account totals are shown in it. */
  baseCurrency?: string
  /** An archived wallet takes no new sub-accounts. */
  archived: boolean
}
