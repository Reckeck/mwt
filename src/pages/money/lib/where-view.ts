import { formatMoney, formatQuantity, isFiatCode } from '@/shared/lib/format'

/** A sum in the units it moved in: «5 000 ₽», «0,04 BTC». */
export function unitText(qty: string, unit: string): string {
  if (isFiatCode(unit)) return formatMoney(Math.abs(Number(qty)), { decimals: 0, currency: unit })
  return `${formatQuantity(Math.abs(Number(qty)))} ${unit.toUpperCase()}`
}

/** «4 500 USDT → 0,05 BTC», or only what left. */
export function exchangeText(item: WhereItem): string {
  const sent = unitText(item.qty, item.unit)
  return item.got_qty && item.got_unit && item.got_unit !== item.unit
    ? `${sent} → ${unitText(item.got_qty, item.got_unit)}`
    : sent
}

/** The one unit every item of a row moved in, if they share one. */
export function sharedUnit(row: WhereRow): string | null {
  const units = new Set(row.items.map((item) => item.unit.toLowerCase()))
  return units.size === 1 ? row.items[0].unit : null
}

/** The rows that open on the spot (Paths 2 · 9): their items are operations, not category totals. */
export const WHERE_OPENS: readonly WhereKind[] = ['lent', 'moved', 'invested']

/** The kinds the operations list filters by to show a row's operations. */
export const WHERE_TX_KINDS: Partial<Record<WhereKind, TxKind[]>> = {
  lent: ['transfer'],
  moved: ['transfer', 'convert'],
  invested: ['trade_spot'],
}

/** One operation of an opened row: «21 окт · Тинькофф → Bybit · 45 000 ₽ → 480 USDT». */
export function whereItemRoute(kind: WhereKind, item: WhereItem): string {
  if (kind === 'lent') return [item.comment || item.label, item.to].filter(Boolean).join(', ')
  if (kind === 'invested') return item.label
  return `${item.label} → ${item.to ?? ''}`
}
