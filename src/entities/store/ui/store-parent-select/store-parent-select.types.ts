export interface StoreParentSelectProps {
  /** Every wallet; the picker shows them as a tree. */
  stores: Store[]
  /** Wallet being moved: it and its sub-wallets cannot become its parent. */
  excludeId?: string | null
}
