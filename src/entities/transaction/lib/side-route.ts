import { txRouteSides } from '@/shared/lib/transactions'

/** One side of `Tinkoff → Bybit`: the account and its name. */
export interface RouteSide {
  side: AccountRef
  label: string
}

/** Where the money went or came from, in parts: arrows are strings, sides objects (each can be its own link). */
export function sideRoute(tx: Transaction, sideLabel: (side: AccountRef) => string): (string | RouteSide)[] {
  const { from, to } = txRouteSides(tx)
  const part = (side: AccountRef) => ({ side, label: sideLabel(side) })
  if (from && to) return [part(from), '→', part(to)]
  if (from) return ['←', part(from)]
  if (to) return ['→', part(to)]
  return []
}

/** The same route as one line. */
export function sideRouteText(parts: (string | RouteSide)[]): string {
  return parts.map((part) => (typeof part === 'string' ? part : part.label)).join(' ')
}

/**
 * A comment that only repeats a side of the route (`Pyaterochka` in `Tinkoff → Pyaterochka`), or another
 * name the row already shows (`also`: its wallet, its title: `Tinkoff cashback` of a cashback), is not said twice.
 */
export function routeRepeats(parts: (string | RouteSide)[], comment: string, ...also: string[]): boolean {
  const text = comment.trim().toLowerCase()
  const labels = [...parts.flatMap((part) => (typeof part === 'string' ? [] : [part.label])), ...also]
  return Boolean(text) && labels.some((label) => label.trim().toLowerCase() === text)
}
