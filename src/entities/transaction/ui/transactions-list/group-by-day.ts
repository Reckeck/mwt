import { txIsInternal, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'

export interface TxDay {
  /** Local `YYYY-MM-DD`. */
  date: string
  txs: Transaction[]
  /** Net money of the day in the base currency: what came in minus what went out. */
  totalBase: number
  /** Some leg had no price: the total leaves it out. */
  partial: boolean
}

/** Local calendar day of an RFC3339 moment. */
function localDay(iso: string): string {
  const date = new Date(iso)
  if (Number.isNaN(date.getTime())) return iso.slice(0, 10)
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

/**
 * Consecutive transactions of one local day, newest day first as the list
 * comes. A leg is valued at its recorded USD price times `fxPerUsd`; a move
 * between the user's own wallets is neither in nor out and is left out.
 * `storeId` counts only that wallet's side.
 */
export function groupByDay(
  txs: Transaction[],
  fxPerUsd: number,
  rootOf: (storeId: string) => string,
  storeId: string | null = null,
): TxDay[] {
  const days: TxDay[] = []
  for (const tx of txs) {
    const date = localDay(txMeta(tx).timestamp)
    let day = days.at(-1)
    if (!day || day.date !== date) {
      day = { date, txs: [], totalBase: 0, partial: false }
      days.push(day)
    }
    day.txs.push(tx)
    if (txIsInternal(tx, rootOf)) continue
    const legs = txPrincipalLegs(tx).filter((leg) => !storeId || leg.store_id === storeId)
    // Half a conversion would read as income or spending: a transaction counts whole or not at all.
    if (legs.some((leg) => !leg.price_usd)) {
      day.partial = true
      continue
    }
    for (const leg of legs) day.totalBase += Number(leg.amount) * Number(leg.price_usd) * fxPerUsd
  }
  return days
}
