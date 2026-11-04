/** The wallet tree: placement rules, positions, previews, overviews. */

import { txLegs } from '@/shared/lib/transactions'
import { mockContacts } from './ledger'
import type { AppError } from '../ipc'
import { mockFilterTxs, mockStores, mockTransactions, priceOf, timestampOf } from './demo'
import { mockAppSettings } from './vault-data'

/** Mirrors `MAX_STORE_DEPTH` in Rust: a root is depth 0, so two levels. */
const MOCK_MAX_DEPTH = 1

export const mockStore = (id: string) => mockStores.find((candidate) => candidate.id === id)
const mockChildren = (id: string) => mockStores.filter((candidate) => candidate.parent_id === id)

/** Another programme uses the same points symbol (`loyalty_symbol_clash` in Rust). */
const mockSymbolClash = (store: Store) =>
  mockStores.some(
    (other) =>
      other.id !== store.id &&
      other.loyalty?.points_symbol.trim().toUpperCase() === store.loyalty?.points_symbol.trim().toUpperCase(),
  )

/** Parent chain from the root down to `id`'s parent. */
export function mockAncestors(id: string): Store[] {
  const chain: Store[] = []
  let parent = mockStore(id)?.parent_id ?? null
  while (parent) {
    const store = mockStore(parent)
    if (!store || chain.includes(store)) break
    chain.unshift(store)
    parent = store.parent_id
  }
  return chain
}

export function mockSubtreeIds(id: string): string[] {
  return mockChildren(id).flatMap((child) => [child.id, ...mockSubtreeIds(child.id)])
}

function mockHeight(id: string): number {
  return Math.max(0, ...mockChildren(id).map((child) => mockHeight(child.id) + 1))
}

/** Same rules and codes as `validate_placement` in Rust (R1–R6). */
export function mockPlacementError(node: string | null, parent: string | null, name: string): AppError | null {
  if (parent) {
    const target = mockStore(parent)
    if (!target) return { code: 'MISSING', message: 'parent store' }
    if (node && !mockStore(node)?.parent_id) return { code: 'STORE_ROOT_MOVE', message: 'root move' }
    if (node && (parent === node || mockSubtreeIds(node).includes(parent)))
      return { code: 'STORE_CYCLE', message: 'cycle' }
    if (mockAncestors(parent).length + 1 + (node ? mockHeight(node) : 0) > MOCK_MAX_DEPTH)
      return { code: 'STORE_TOO_DEEP', message: 'too deep' }
    if (target.archived) return { code: 'STORE_PARENT_ARCHIVED', message: 'parent archived' }
  }
  const wanted = name.trim().toLowerCase()
  const taken = mockStores.some(
    (store) => store.id !== node && store.parent_id === parent && store.name.trim().toLowerCase() === wanted,
  )
  return taken ? { code: 'STORE_NAME_TAKEN', message: 'name taken' } : null
}

/** When the mock's quotes were «taken»: a few minutes ago. */
const mockPricesAsOf = new Date(Date.now() - 4 * 60_000).toISOString()

/** Positions held on the store itself, like `store_positions` in Rust. */
export function mockOwnPositions(storeId: string): StorePosition[] {
  const byAsset = new Map<string, { asset: AssetRef; qty: number; cost: number }>()
  // Legs carry their own sign, so a trade or transfer needs no special case.
  for (const leg of mockTransactions.flatMap(txLegs)) {
    if (leg.store_id !== storeId || leg.asset.contract?.startsWith('perp:')) continue
    const key = leg.asset.symbol.toUpperCase()
    const row = byAsset.get(key) ?? { asset: leg.asset, qty: 0, cost: 0 }
    row.qty += Number(leg.amount)
    if (Number(leg.amount) > 0) row.cost = priceOf(key)
    byAsset.set(key, row)
  }
  return [...byAsset.values()]
    .filter((row) => row.qty !== 0)
    .map((row) => {
      const priceUsd = priceOf(row.asset.symbol)
      return {
        asset: row.asset,
        quantity: String(row.qty),
        avg_cost: row.cost ? String(row.cost) : null,
        price_usd: String(priceUsd),
        price_base: String(priceUsd),
        value_usd: String(row.qty * priceUsd),
        value_base: String(row.qty * priceUsd),
        unrealized_base: String(row.qty * (priceUsd - row.cost)),
        // A fiat unit is priced by its rate; the rest look like a CoinGecko quote moving a little.
        change_24h_pct: row.asset.chain === 'fiat' ? null : String(((row.asset.symbol.charCodeAt(0) % 7) - 3) * 0.8),
        price_info:
          row.asset.chain === 'fiat'
            ? { origin: 'fiat' as const, source: 'open.er-api', at: mockPricesAsOf }
            : { origin: 'market' as const, source: 'coingecko', at: mockPricesAsOf },
      }
    })
    .sort((left, right) => Number(right.value_base) - Number(left.value_base))
}

const sumValue = (positions: StorePosition[]) => positions.reduce((acc, row) => acc + Number(row.value_usd), 0)

/** Like `tree_balances` in Rust: property in its own currency, sub-accounts merged by ticker. */
function mockBalances(store: Store, own: number, children: StoreTreeNode[]): AssetBalance[] {
  const code = store.property?.currency?.toUpperCase()
  const rate = code ? priceOf(code) : 0
  const mine: AssetBalance[] =
    code && rate
      ? [{ asset: fiatRef(code), quantity: String(own / rate), value_base: String(own) }]
      : mockOwnPositions(store.id)
          .filter((row) => Number(row.quantity) > 0)
          .map((row) => ({ asset: row.asset, quantity: row.quantity, value_base: row.value_base }))
  for (const balance of children.flatMap((child) => child.balances)) {
    const seen = mine.find((row) => row.asset.symbol.toUpperCase() === balance.asset.symbol.toUpperCase())
    if (!seen) mine.push({ ...balance })
    else {
      seen.quantity = String(Number(seen.quantity) + Number(balance.quantity))
      seen.value_base = String(Number(seen.value_base) + Number(balance.value_base))
    }
  }
  return mine.sort((left, right) => Number(right.value_base) - Number(left.value_base))
}

const fiatRef = (symbol: string): AssetRef => ({
  symbol,
  name: symbol,
  coingecko_id: null,
  chain: 'fiat',
  contract: null,
  quote: symbol,
})

export function mockTreeNode(store: Store, depth: number, parentPath: string[]): StoreTreeNode {
  const path = [...parentPath, store.name]
  const children = mockChildren(store.id)
    .map((child) => mockTreeNode(child, depth + 1, path))
    .sort((left, right) => left.store.name.localeCompare(right.store.name))
  const own = sumValue(mockOwnPositions(store.id))
  const subtree = children.reduce((acc, child) => acc + Number(child.subtree_usd), own)
  return {
    store,
    depth,
    path,
    own_usd: String(own),
    own_base: String(own),
    subtree_usd: String(subtree),
    subtree_base: String(subtree),
    own_tx_count: mockFilterTxs([store.id]).length,
    can_add_child:
      depth < MOCK_MAX_DEPTH && !store.archived && !['property', 'cash', 'loyalty', 'liability'].includes(store.kind),
    children,
    // Like the core: own biggest first, then the sub-accounts', one per ticker.
    top_assets: [
      ...mockOwnPositions(store.id)
        .filter((row) => Number(row.quantity) > 0)
        .map((row) => row.asset),
      ...children.flatMap((child) => child.top_assets),
    ]
      .filter((asset, index, list) => list.findIndex((other) => other.symbol === asset.symbol) === index)
      .slice(0, 3),
    balances: mockBalances(store, own, children),
    last_tx_at:
      [...mockFilterTxs([store.id]).map(timestampOf), ...children.map((child) => child.last_tx_at ?? '')]
        .filter(Boolean)
        .sort()
        .at(-1) ?? null,
  }
}

export function mockTree(): StoreTreeNode[] {
  return mockStores
    .filter((store) => !store.parent_id || !mockStore(store.parent_id))
    .map((store) => mockTreeNode(store, 0, []))
    .sort((left, right) => left.store.name.localeCompare(right.store.name))
}

function mockFindNode(nodes: StoreTreeNode[], id: string): StoreTreeNode | undefined {
  for (const node of nodes) {
    if (node.store.id === id) return node
    const found = mockFindNode(node.children, id)
    if (found) return found
  }
  return undefined
}

const flatNodes = (nodes: StoreTreeNode[]): StoreTreeNode[] =>
  nodes.flatMap((node) => [{ ...node, children: [] }, ...flatNodes(node.children)])

export function mockMoveTargets(id: string): StoreMoveTargets {
  const store = mockStore(id)
  if (!store?.parent_id) return { parents: [], can_detach: false }
  return {
    parents: flatNodes(mockTree()).filter(
      (node) => node.store.id !== store.parent_id && !mockPlacementError(id, node.store.id, store.name),
    ),
    can_detach: !mockPlacementError(id, null, store.name),
  }
}

/** Every id a store scope covers. */
const mockScope = (storeId: string, includeChildren: boolean) =>
  includeChildren ? [storeId, ...mockSubtreeIds(storeId)] : [storeId]

export const mockExploreCtx = () => ({
  transactions: mockTransactions,
  tree: mockTree(),
  contacts: mockContacts,
  scope: mockScope,
})

export function mockOverview(storeId: string, includeChildren: boolean): StoreOverview {
  const store = mockStore(storeId) ?? mockStores[0]
  const node = mockFindNode(mockTree(), store.id) ?? mockTreeNode(store, 0, [])
  const txs = mockFilterTxs(mockScope(store.id, includeChildren))
  // Merged per asset like `merged_positions` in Rust (cost basis left out).
  const merged = new Map<string, StorePosition>()
  for (const row of [store.id, ...mockSubtreeIds(store.id)].flatMap(mockOwnPositions)) {
    const key = row.asset.symbol.toUpperCase()
    const prev = merged.get(key)
    const add = (left: string, right: string) => String(Number(left) + Number(right))
    merged.set(
      key,
      prev
        ? {
            ...prev,
            avg_cost: null,
            quantity: add(prev.quantity, row.quantity),
            value_usd: add(prev.value_usd, row.value_usd),
            value_base: add(prev.value_base, row.value_base),
            unrealized_base: add(prev.unrealized_base, row.unrealized_base),
          }
        : row,
    )
  }
  const subtreePositions = [...merged.values()].sort(
    (left, right) => Number(right.value_base) - Number(left.value_base),
  )
  const subtreeTotal = Number(node.subtree_usd)
  const own = mockOwnPositions(store.id)
  return {
    store,
    ancestors: mockAncestors(store.id).map((parent) => ({ id: parent.id, name: parent.name, color: parent.color })),
    base_currency: mockAppSettings.base_currency,
    fx_per_usd: '1',
    include_children: includeChildren,
    own_usd: node.own_usd,
    own_base: node.own_base,
    subtree_usd: node.subtree_usd,
    subtree_base: node.subtree_base,
    realized_pnl_base: '0',
    positions: own,
    subtree_positions: subtreePositions,
    open_positions: [],
    children: node.children.map((child) => ({
      store: child.store,
      value_usd: child.own_usd,
      value_base: child.own_base,
      subtree_base: child.subtree_base,
      share: subtreeTotal ? Number(child.subtree_usd) / subtreeTotal : 0,
    })),
    can_add_child: node.can_add_child,
    tx_count: txs.length,
    first_tx_at: txs.length ? timestampOf(txs[txs.length - 1]) : '',
    last_tx_at: txs.length ? timestampOf(txs[0]) : '',
    unrealized_base: String(own.reduce((sum, row) => sum + Number(row.unrealized_base), 0)),
    subtree_unrealized_base: String(subtreePositions.reduce((sum, row) => sum + Number(row.unrealized_base), 0)),
    prices_as_of: own.length ? mockPricesAsOf : null,
    symbol_clash: store.loyalty ? mockSymbolClash(store) : false,
  }
}
