import { emptyTxFilter } from '@/shared/lib/tx-filter'

import { txKind, txMeta } from './access'
import { txPrincipalLegs } from './legs'

/** What was given and what came: the two sides of an exchange. */
export interface TxPair {
  out: { amount: number; symbol: string }
  into: { amount: number; symbol: string }
}

const TWO_SIDED: TxKind[] = ['convert', 'trade_spot']

export function txPair(tx: Transaction): TxPair | null {
  const legs = txPrincipalLegs(tx)
  const out = legs.find((leg) => Number(leg.amount) < 0)
  const into = legs.find((leg) => Number(leg.amount) > 0 && leg.asset.symbol !== out?.asset.symbol)
  if (!out || !into) return null
  return {
    out: { amount: Math.abs(Number(out.amount)), symbol: out.asset.symbol },
    into: { amount: Number(into.amount), symbol: into.asset.symbol },
  }
}

/**
 * The list that holds operations like this one: the same exchange pair, or
 * payments of the same category. `null` when nothing is alike by rule.
 */
export function similarFilter(tx: Transaction): TxFilter | null {
  const kind = txKind(tx)
  const pair = txPair(tx)
  if (TWO_SIDED.includes(kind) && pair) return emptyTxFilter({ kinds: [kind], text: pair.into.symbol })
  const category = txMeta(tx).categories[0]
  if (kind === 'payment' && category) return emptyTxFilter({ kinds: ['payment'], category })
  return null
}
