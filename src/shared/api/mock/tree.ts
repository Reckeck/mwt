/** The wallet tree: placement rules, positions, previews, overviews. */

import { txId, txLegs } from '@/shared/lib/transactions'
import { mockContacts, mockTransactionFrom } from './ledger'
import type { AppError } from '../ipc'
import { mockFilterTxs, mockStores, mockTransactions, priceOf, timestampOf } from './demo'
import { mockSummary } from './market'
import { mockAppSettings } from './vault-data'

/** Mirrors `MAX_STORE_DEPTH` in Rust: a root is depth 0, so two levels. */
const MOCK_MAX_DEPTH = 1

export const mockStore = (id: string) => mockStores.find((candidate) => candidate.id === id)
const mockChildren = (id: string) => mockStores.filter((candidate) => candidate.parent_id === id)

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
      }
    })
    .sort((left, right) => Number(right.value_base) - Number(left.value_base))
}

/** Like `preview_transaction` in Rust: net legs per store and asset, nothing written. */
export function mockPreview(input: PreviewTransactionInput): TxPreview {
  // Like the backend: a draft that would be refused (no category, no comment)
  // still previews its balances, with the refusal reported in `error`.
  let error: string | null = null
  let draft: Transaction
  try {
    draft = mockTransactionFrom(input.transaction, priceOf)
  } catch (err) {
    const failure = err as { code?: string; message?: string }
    error = `${failure.code ?? 'INVALID_INPUT'}: ${failure.message ?? ''}`
    const common = input.transaction.common
    const patched = {
      ...input.transaction,
      common: {
        ...common,
        comment: common.comment || 'preview',
        categories: common.categories.length ? common.categories : ['other'],
      },
    } as TransactionInput
    try {
      draft = mockTransactionFrom(patched, priceOf)
    } catch {
      return { changes: [], base_currency: mockSummary.base_currency, flow: 'ignored', warnings: [], error }
    }
  }
  const replaced = input.replaces ? mockTransactions.find((tx) => txId(tx) === input.replaces) : undefined
  const deltas = new Map<string, { storeId: string; asset: AssetRef; delta: number }>()
  const add = (legs: ReturnType<typeof txLegs>, sign: number) => {
    for (const leg of legs) {
      const key = `${leg.store_id}:${leg.asset.symbol.toUpperCase()}`
      const row = deltas.get(key) ?? { storeId: leg.store_id, asset: leg.asset, delta: 0 }
      row.delta += sign * Number(leg.amount)
      deltas.set(key, row)
    }
  }
  add(txLegs(draft), 1)
  if (replaced) add(txLegs(replaced), -1)
  const changes = [...deltas.values()]
    .filter((row) => row.delta !== 0)
    .map((row) => {
      const held = mockOwnPositions(row.storeId).find(
        (position) => position.asset.symbol.toUpperCase() === row.asset.symbol.toUpperCase(),
      )
      const before = Number(held?.quantity ?? 0)
      return {
        store_id: row.storeId,
        store_name: mockStore(row.storeId)?.name ?? '',
        asset: row.asset,
        before: String(before),
        after: String(before + row.delta),
        delta_base: String(row.delta * priceOf(row.asset.symbol)),
      }
    })
  const warnings: PreviewWarning[] = changes
    .filter((change) => Number(change.after) < 0 && mockStore(change.store_id)?.kind !== 'liability')
    .map((change) => ({ kind: 'negative_balance', store_id: change.store_id, asset: change.asset }))
  const flow: FlowClass = 'payment' in draft ? 'daily' : 'topup' in draft ? 'income' : 'ignored'
  return { changes, base_currency: mockSummary.base_currency, flow, warnings, error }
}

const sumValue = (positions: StorePosition[]) => positions.reduce((acc, row) => acc + Number(row.value_usd), 0)

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
    positions: mockOwnPositions(store.id),
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
  }
}
