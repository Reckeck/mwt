/** What the ledger holds, split like the core splits net worth (financial / crypto / property / debt). */

import { txLegs, txMeta } from '@/shared/lib/transactions'
import type { AnalyticsMockContext } from './flows'

export interface WorthSplit {
  financial: number
  crypto: number
  non_financial: number
  liabilities: number
}

/** Every leg a store holds up to `month` (all time when `null`), valued at today's prices. */
export function ledgerWorth(ctx: AnalyticsMockContext, month: string | null, scope: string[] = []): WorthSplit {
  const kinds = new Map(ctx.stores.map((store) => [store.id, store.kind]))
  const split: WorthSplit = { financial: 0, crypto: 0, non_financial: 0, liabilities: 0 }
  for (const tx of ctx.transactions) {
    if (month !== null && txMeta(tx).timestamp.slice(0, 7) > month) continue
    for (const leg of txLegs(tx)) {
      if (leg.asset.contract?.startsWith('perp:') || (scope.length && !scope.includes(leg.store_id))) continue
      const value = Number(leg.amount) * ctx.priceOf(leg.asset.symbol)
      const kind = kinds.get(leg.store_id)
      if (kind === 'property' || kind === 'loyalty') split.non_financial += value
      else if (kind === 'liability') split.liabilities -= value
      else {
        split.financial += value
        if (kind === 'crypto' || kind === 'exchange') split.crypto += value
      }
    }
  }
  return split
}
