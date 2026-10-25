import { txSides } from '@/shared/lib/transactions'

/** One side of «Тинькофф → Bybit»: the account and its name. */
export interface RouteSide {
  side: AccountRef
  label: string
}

/** Where the money went or came from, in parts: arrows are strings, sides objects (each can be its own link). */
export function sideRoute(tx: Transaction, sideLabel: (side: AccountRef) => string): (string | RouteSide)[] {
  const { from, to } = txSides(tx)
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
