/** Node colour per kind: the four verbs stand apart, trading shares warm tones. */
export const KIND_COLORS: Record<TxKind, string> = {
  topup: '#16a34a',
  payment: '#dc2626',
  transfer: '#2563eb',
  convert: '#7c3aed',
  trade_spot: '#d97706',
  trade_futures: '#ea580c',
  defi: '#0891b2',
  asset_sell: '#64748b',
}

/** Edge look per kind: a lane's own sequence is quiet, moves between lanes are not. */
export const EDGE_STYLES: Record<
  GraphEdgeKind,
  { color: string; width: number; type: 'solid' | 'dashed'; curveness: number }
> = {
  seq: { color: '#94a3b8', width: 1, type: 'solid', curveness: 0 },
  transfer: { color: '#2563eb', width: 1.5, type: 'solid', curveness: 0 },
  convert: { color: '#7c3aed', width: 1.5, type: 'dashed', curveness: 0 },
  flow: { color: '#f59e0b', width: 2, type: 'dashed', curveness: 0.3 },
}

/** Pixels per lane; the chart grows with the number of lanes. */
export const LANE_HEIGHT = 44
