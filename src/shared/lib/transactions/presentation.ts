/** How a transaction looks in lists: badge tone per kind, colour of an amount. */

import { signClass } from '../format'

export type TxBadgeVariant = 'default' | 'secondary' | 'destructive' | 'outline'

/** Badge tone per kind (shared by the transaction list and a store's history). */
export const TX_KIND_BADGE: Record<TxKind, TxBadgeVariant> = {
  topup: 'default',
  payment: 'destructive',
  transfer: 'secondary',
  convert: 'secondary',
  trade_spot: 'secondary',
  trade_futures: 'secondary',
  defi: 'secondary',
  asset_sell: 'secondary',
  adjustment: 'outline',
}

export const DEFAULT_TX_BADGE: TxBadgeVariant = 'secondary'

/** Colour of a signed amount: green in, red out, neutral for zero or none. */
export function amountClass(signed: string): string {
  return signClass(signed)
}

/** Colour for a whole transaction, from the kinds that only ever move money one way. */
export function txAmountClass(kind: TxKind): string {
  if (kind === 'topup') return 'text-success'
  return kind === 'payment' ? 'text-destructive' : ''
}
