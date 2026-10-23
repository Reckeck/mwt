import { txKind, txPrincipalLegs, txSides } from '@/shared/lib/transactions'

/** One part of «−4 500 USDT · +0,05 BTC · 0,04 BTC». */
export interface BulkTotal {
  symbol: string
  amount: number
  /** A move between own wallets is neither in nor out: no sign. */
  signed: boolean
}

/** What the picked operations add up to, per asset, the way their rows show them. */
export function bulkTotals(txs: readonly Transaction[]): BulkTotal[] {
  const totals = new Map<string, BulkTotal>()
  for (const tx of txs) {
    const { from, to } = txSides(tx)
    const own = txKind(tx) === 'transfer' && Boolean(from && to && 'store' in from && 'store' in to)
    const legs = txPrincipalLegs(tx).filter((leg) => !own || Number(leg.amount) > 0)
    for (const leg of legs) {
      const key = `${own ? 'move' : 'flow'}:${leg.asset.symbol}`
      const total = totals.get(key) ?? { symbol: leg.asset.symbol, amount: 0, signed: !own }
      total.amount += Number(leg.amount)
      totals.set(key, total)
    }
  }
  return [...totals.values()].filter((total) => total.amount !== 0)
}

/** Only a payment or a top-up has one wallet to move to. */
export function movable(tx: Transaction): boolean {
  return 'payment' in tx || 'topup' in tx
}
