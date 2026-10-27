import { flattenStoreTree } from '@/shared/lib/store-tree'

/** One tile of the sub-accounts row: the wallet itself («Свои средства») or one of its sub-accounts. */
export interface SubAccountTile {
  id: string
  store: Store
  /** What it holds in its own currency (the biggest one when there are several). */
  amount: string
  currency: string
  /** Further currencies held beyond the one shown. */
  extra: number
  /** The same value in the base currency. */
  baseAmount: string
  /** Part of the wallet's subtree value, 0..1. */
  share: number
  /** The wallet itself: the page being open. */
  own: boolean
}

export interface SubAccountTilesInput {
  store: Store
  children: StoreChildRow[]
  ownBase: string
  ownShare: number
  ownPositions: StorePosition[]
  base: string
  tree: StoreTreeNode[]
}

/** Own balance first, then each direct sub-account in the order the backend sorted them. */
export function buildSubAccountTiles(input: SubAccountTilesInput): SubAccountTile[] {
  const nodes = flattenStoreTree(input.tree)
  const held = input.ownPositions.filter((position) => Number(position.quantity) !== 0)
  const [first] = held
  const own: SubAccountTile = {
    id: input.store.id,
    store: input.store,
    amount: first ? first.quantity : input.ownBase,
    currency: first ? first.asset.symbol : input.base,
    extra: Math.max(0, held.length - 1),
    baseAmount: input.ownBase,
    share: input.ownShare,
    own: true,
  }
  const subs = input.children.map((row): SubAccountTile => {
    const balances = nodes.find((node) => node.store.id === row.store.id)?.balances ?? []
    const [top] = balances
    return {
      id: row.store.id,
      store: row.store,
      amount: top ? top.quantity : row.subtree_base,
      currency: top ? top.asset.symbol : input.base,
      extra: Math.max(0, balances.length - 1),
      baseAmount: row.subtree_base,
      share: row.share,
      own: false,
    }
  })
  return [own, ...subs]
}
