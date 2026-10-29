import { formatDayMonthShort } from '@/shared/lib/format'
import { txSides } from '@/shared/lib/transactions'

export interface GlancePayment {
  id: string
  name: string
  day: string
  /** The sum for `<Money :amount :currency :at>`: the template draws it. */
  amount: string
  currency: string
  at: string
}

/**
 * `Perekrestok · Oct 14 9 $`: a payment in a glance's `Latest payments` —
 * the comment, else who was paid, else the category.
 */
export function glancePayments(
  items: Transaction[],
  names: { side: (side: AccountRef) => string; category: (key: string) => string },
): GlancePayment[] {
  return items.flatMap((tx) => {
    if (!('payment' in tx)) return []
    const body = tx.payment
    const { to } = txSides(tx)
    const name = body.meta.comment || (to ? names.side(to) : names.category(body.meta.categories[0] ?? ''))
    return [
      {
        id: body.meta.id,
        name,
        day: formatDayMonthShort(body.meta.timestamp),
        amount: body.amount,
        currency: body.asset.symbol,
        at: body.meta.timestamp,
      },
    ]
  })
}
