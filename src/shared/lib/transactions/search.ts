/** Free-text search over a transaction. */

import { txMeta } from './access'
import { txChainAndHash } from './explorer'
import { txDetails } from './facts'
import { txLegs } from './legs'
import { txSides } from './sides'

/** Everything a free-text search should match: assets, comment, categories, hash, facts. */
export function txSearchText(tx: Transaction): string {
  const meta = txMeta(tx)
  const sides = txSides(tx)
  const raw = [sides.from, sides.to].map((side) => (side && 'raw' in side ? side.raw : ''))
  return [
    ...txLegs(tx).map((leg) => leg.asset.symbol),
    meta.comment,
    ...meta.categories,
    ...raw,
    txChainAndHash(tx).hash ?? '',
    ...txDetails(tx).map(([, value]) => value),
  ]
    .join(' ')
    .toLowerCase()
}
