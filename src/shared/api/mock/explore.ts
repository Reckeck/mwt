/**
 * Browser-mock half of the operations list and graph for `pnpm dev`: the same
 * filter and lane layout as `service/ledger.rs`, in local time instead of the
 * user's zone, with values from the stamped USD price.
 */

import { txContactIds, txKind, txLegs, txMeta, txSearchText, txSides } from '@/shared/lib/transactions'

export interface ExploreMockContext {
  transactions: Transaction[]
  tree: StoreTreeNode[]
  contacts: Contact[]
  /** Store ids a scope entry covers. */
  scope: (storeId: string, includeChildren: boolean) => string[]
}

const GRAPH_NODE_CAP = 2000

function flatten(nodes: StoreTreeNode[]): StoreTreeNode[] {
  return nodes.flatMap((node) => [node, ...flatten(node.children)])
}

function storesOf(tx: Transaction): string[] {
  const sides = txSides(tx)
  const out: string[] = []
  for (const side of [sides.from, sides.to])
    if (side && 'store' in side && !out.includes(side.store)) out.push(side.store)
  for (const leg of txLegs(tx)) {
    for (const id of [leg.store_id, leg.to_store_id]) if (id && !out.includes(id)) out.push(id)
  }
  return out
}

function rootOf(tree: StoreTreeNode[], storeId: string): string {
  return tree.find((root) => flatten([root]).some((node) => node.store.id === storeId))?.store.id ?? storeId
}

function isInternal(tx: Transaction, tree: StoreTreeNode[]): boolean {
  if (!('transfer' in tx) && !('convert' in tx)) return false
  const sides = txSides(tx)
  if (!sides.from || !sides.to || !('store' in sides.from) || !('store' in sides.to)) return false
  return rootOf(tree, sides.from.store) === rootOf(tree, sides.to.store)
}

const localDay = (iso: string) => {
  const date = new Date(iso)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

/** The larger of what left and what arrived, by principal legs, in USD. */
function usdValueOf(tx: Transaction): number {
  let gave = 0
  let got = 0
  for (const leg of txLegs(tx).filter((leg) => leg.role === 'principal')) {
    const value = Math.abs(Number(leg.amount)) * (Number(leg.price_usd) || 0)
    if (Number(leg.amount) < 0) gave += value
    else got += value
  }
  return Math.max(gave, got)
}

function filtered(filter: TxFilter, ctx: ExploreMockContext): Transaction[] {
  const ids = filter.scope.store_ids.length
    ? new Set(filter.scope.store_ids.flatMap((id) => ctx.scope(id, filter.scope.include_children)))
    : null
  const needle = filter.text?.trim().toLowerCase() ?? ''
  const nameOf = (id: string) => flatten(ctx.tree).find((node) => node.store.id === id)?.store.name ?? ''
  return ctx.transactions.filter((tx) => {
    if (ids && !storesOf(tx).some((id) => ids.has(id))) return false
    if (filter.kinds.length && !filter.kinds.includes(txKind(tx))) return false
    if (filter.contact_id && !txContactIds(tx).includes(filter.contact_id)) return false
    if (filter.flow_id && txMeta(tx).flow_id !== filter.flow_id) return false
    if (filter.with_fee && !txLegs(tx).some((leg) => leg.role === 'fee')) return false
    // The mock keeps no cost basis: «closed a position» is a spot sale or an asset sale.
    if (filter.realized && !['trade_spot', 'asset_sell'].includes(txKind(tx))) return false
    const origin = txMeta(tx).origin
    if (filter.auto_only && (origin === 'manual' || 'import' in origin)) return false
    if (filter.debt_only && !('transfer' in tx && tx.transfer.debt)) return false
    if (filter.amount_min || filter.amount_max) {
      const value = usdValueOf(tx)
      if (filter.amount_min && value < Number(filter.amount_min)) return false
      if (filter.amount_max && value > Number(filter.amount_max)) return false
    }
    const day = localDay(txMeta(tx).timestamp)
    if (filter.from && day < filter.from) return false
    if (filter.to && day > filter.to) return false
    if (filter.hide_internal && isInternal(tx, ctx.tree)) return false
    if (filter.category !== null && (txMeta(tx).categories[0] ?? '') !== filter.category) return false
    if (!needle) return true
    const names = [
      ...storesOf(tx).map(nameOf),
      ...txContactIds(tx).map((id) => ctx.contacts.find((contact) => contact.id === id)?.name ?? ''),
    ]
    return `${txSearchText(tx)} ${names.join(' ').toLowerCase()}`.includes(needle)
  })
}

export function mockListTxs(filter: TxFilter, ctx: ExploreMockContext): Transaction[] {
  return filtered(filter, ctx).sort((left, right) => txMeta(right).timestamp.localeCompare(txMeta(left).timestamp))
}

export function mockTxGraph(filter: TxFilter, collapsed: string[], ctx: ExploreMockContext): TxGraph {
  const txs = filtered(filter, ctx).sort((left, right) => txMeta(left).timestamp.localeCompare(txMeta(right).timestamp))
  const order = flatten(ctx.tree)
  const scope = filter.scope.store_ids.length
    ? new Set(filter.scope.store_ids.flatMap((id) => ctx.scope(id, filter.scope.include_children)))
    : null
  const laneOf = (storeId: string): string | null => {
    if (scope && !scope.has(storeId)) return null
    const folded = collapsed.find((id) => id !== storeId && ctx.scope(id, true).includes(storeId))
    return folded ?? storeId
  }
  const occurrences = txs.map((tx) => [
    ...new Set(
      storesOf(tx)
        .map(laneOf)
        .filter((lane) => lane !== null),
    ),
  ])
  const count = occurrences.reduce((sum, lanes) => sum + lanes.length, 0)
  if (count > GRAPH_NODE_CAP) throw { code: 'TOO_LARGE', message: `${count} nodes` }

  const used = new Set(occurrences.flat())
  const lanes: GraphLane[] = order
    .filter((node) => used.has(node.store.id))
    .map((node) => ({
      store_id: node.store.id,
      name: node.path.join(' › '),
      root_id: rootOf(ctx.tree, node.store.id),
      depth: node.depth,
      has_children: node.children.length > 0,
      collapsed: collapsed.includes(node.store.id),
    }))
  const laneIndex = new Map(lanes.map((lane, index) => [lane.store_id, index]))
  const nodes: GraphNode[] = []
  const edges: GraphEdge[] = []
  const lastOnLane = new Map<string, string>()
  const lastInFlow = new Map<string, string>()
  let column = 0
  txs.forEach((tx, index) => {
    const lanesOfTx = occurrences[index]
    if (!lanesOfTx.length) return
    const meta = txMeta(tx)
    const ids = lanesOfTx.map((lane) => `${meta.id}:${lane}`)
    const value = txLegs(tx)
      .filter((leg) => leg.role === 'principal' && Number(leg.amount) < 0)
      .reduce((sum, leg) => sum + Math.abs(Number(leg.amount)) * (Number(leg.price_usd) || 0), 0)
    lanesOfTx.forEach((lane, at) => {
      nodes.push({
        id: ids[at],
        tx_id: meta.id,
        kind: txKind(tx),
        timestamp: meta.timestamp,
        lane: laneIndex.get(lane) ?? 0,
        column,
        value: String(Math.round(value * 100) / 100),
        flow_id: meta.flow_id,
        internal: isInternal(tx, ctx.tree),
        comment: meta.comment,
      })
      const previous = lastOnLane.get(lane)
      if (previous) edges.push({ source: previous, target: ids[at], kind: 'seq' })
      lastOnLane.set(lane, ids[at])
    })
    const across = txKind(tx) === 'convert' ? 'convert' : 'transfer'
    for (let at = 1; at < ids.length; at++) edges.push({ source: ids[at - 1], target: ids[at], kind: across })
    if (meta.flow_id) {
      const previous = lastInFlow.get(meta.flow_id)
      if (previous) edges.push({ source: previous, target: ids[0], kind: 'flow' })
      lastInFlow.set(meta.flow_id, ids[ids.length - 1])
    }
    column++
  })
  return { currency: 'usd', lanes, nodes, edges, columns: column }
}
