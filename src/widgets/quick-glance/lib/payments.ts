import { formatDayMonthShort, formatMoney } from '@/shared/lib/format'
import { txSides } from '@/shared/lib/transactions'

export interface GlancePayment {
  id: string
  name: string
  day: string
  amount: string
}

/**
 * «Перекрёсток · 14 окт 9 $»: a payment in a glance's «Последние оплаты» —
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
    const amount = formatMoney(body.amount, { decimals: 0, currency: body.asset.symbol })
    return [{ id: body.meta.id, name, day: formatDayMonthShort(body.meta.timestamp), amount }]
  })
}
