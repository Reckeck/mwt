/** What a toast needs to say about a purchase besides the backend's answer. */
export interface PurchaseChange {
  input: { event_id: string; purchase: SharedPurchaseDraft }
  /** Ids of the purchases there were, to find the new one for `Undo`. */
  knownIds: string[]
  /** The item's name, for the toast. */
  title: string
  /** The wallet's name when the purchase made a payment of mine. */
  walletName?: string
  currency: string
}

export interface SplitChange {
  input: SetSharedItemSplitInput
  /** The split before, for `Undo`. */
  previous: ItemSplit
}

export interface ArchiveChange {
  id: string
  archived: boolean
  title: string
}
