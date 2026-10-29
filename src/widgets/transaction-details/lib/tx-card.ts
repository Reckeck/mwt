import { type TxPair, txKind, txMeta, txPair, txPrincipalLegs } from '@/shared/lib/transactions'

export { type TxPair, similarFilter, txPair } from '@/shared/lib/transactions'

/** Units of the given asset per one received: `93.75 ₽ per USDT`. */
export function pairRate(pair: TxPair): number {
  return pair.into.amount ? pair.out.amount / pair.into.amount : 0
}

/** What `All like this` names: the pair of an exchange, else the category the filter follows. */
export function similarTitle(tx: Transaction, categoryName: (key: string) => string): string {
  const pair = txPair(tx)
  if (pair) return `${pair.out.symbol} → ${pair.into.symbol}`
  const first = txMeta(tx).categories[0]
  return first ? categoryName(first) : ''
}

/** Net money of the operation in the base currency; `null` when a leg has no price. */
export function txBaseValue(tx: Transaction, fxPerUsd: number): number | null {
  // An exchange is worth what it brought: only the received side carries a price.
  const into = txPrincipalLegs(tx).find((leg) => Number(leg.amount) > 0)
  if (txPair(tx) && into) return into.price_usd ? Number(into.amount) * Number(into.price_usd) * fxPerUsd : null
  const legs = txPrincipalLegs(tx)
  if (legs.some((leg) => !leg.price_usd)) return null
  return legs.reduce((total, leg) => total + Number(leg.amount) * Number(leg.price_usd) * fxPerUsd, 0)
}

/** Whether the operation counts in spending, income or neither (`not spending`). */
export function txFlowKind(tx: Transaction): 'spending' | 'income' | 'neutral' {
  const kind = txKind(tx)
  if (kind === 'payment') return 'spending'
  if (kind === 'topup') return 'income'
  return 'neutral'
}

/** A past operation is alike when it is the same pair (an exchange) or passes the filter (a payment). */
export function isSimilar(tx: Transaction, other: Transaction): boolean {
  const pair = txPair(tx)
  if (!pair) return true
  const theirs = txPair(other)
  return theirs?.out.symbol === pair.out.symbol && theirs.into.symbol === pair.into.symbol
}

/** How far this rate is from the average of past ones, in percent; `null` without past ones. */
export function rateDelta(rate: number, past: number[]): number | null {
  const valid = past.filter((value) => value > 0)
  if (!rate || !valid.length) return null
  const average = valid.reduce((total, value) => total + value, 0) / valid.length
  return (rate / average - 1) * 100
}
