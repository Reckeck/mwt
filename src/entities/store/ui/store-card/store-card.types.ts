export interface StoreCardProps {
  store: Store
  /** Set for a sub-wallet: shown as "in <parent>". */
  parent?: Store | null
}
