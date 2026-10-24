import { formatMoney, formatQuantity } from '@/shared/lib/format'
import { fillFormula, prettyFormula } from '@/shared/lib/utility-formula'

import type { BillRow } from '../utility-bill.types'
import { typedReadings } from './price-row'

type T = (key: string, values?: Record<string, unknown>) => string

/** The rent an automation already pays: left out, so it is not paid twice. */
export const automated = (row: BillRow) =>
  'housing' in row.line.item.kind && Boolean(row.line.item.kind.housing.automation_id)

/** A meter whose reading this month is already stored in its card: nothing to type. */
export function readInCard(row: BillRow) {
  const kind = row.line.item.kind
  if (!('metered' in kind)) return false
  return kind.metered.zones.length
    ? row.line.zones.length > 0 && row.line.zones.every((zone) => zone.reading !== null)
    : row.line.reading !== null
}

const quantity = (value: string | number) => formatQuantity(String(value), 3)

/**
 * The line under an item's name (Wallets-2 · 13): «было 13 395 / 5 120 · 6,50 ₽
 * за кВт·ч», «показание 418 записано в карточке · 6 м³ × 49 ₽», «было 205 ·
 * своя формула: …», «как в прошлый раз · можно изменить», or why the rent is off.
 */
export function lineNote(row: BillRow, t: T, automation: string | null): string {
  const { item } = row.line
  const money = (value: string) => formatMoney(value, { currency: item.currency, decimals: Number(value) % 1 ? 2 : 0 })
  const paid = Number(row.line.paid) > 0 ? `${t('utilityBill.alreadyPaid', { amount: money(row.line.paid) })} · ` : ''
  if ('housing' in item.kind) {
    const day = item.kind.housing.due_day
    if (automated(row))
      return automation
        ? t('utilityBill.automatedNamed', { name: automation, day })
        : t('utilityBill.automated', { day })
    return paid + (row.line.includes_deposit ? t('utilities.depositIncluded') : t('utilityBill.asLastTime'))
  }
  if (!('metered' in item.kind)) return paid + t('utilityBill.asLastTime')
  const { unit, tariff, formula, zones } = item.kind.metered
  const price = formula
    ? t('utilityBill.formula', { formula: prettyFormula(formula) })
    : t('utilityTab.tariff', { price: money(tariff), unit })
  if (readInCard(row)) {
    const read = zones.length
      ? row.line.zones.map((zone) => quantity(zone.reading ?? '0')).join(' / ')
      : quantity(row.line.reading ?? '0')
    const used = row.line.consumption === null ? null : `${quantity(row.line.consumption)} ${unit}`
    const how = formula ? t('utilityTab.ownFormula') : used ? `${used} × ${money(tariff)}` : price
    const stored = t(zones.length ? 'utilityBill.readInCardMany' : 'utilityBill.readInCard', { value: read })
    return `${paid}${stored} · ${how}`
  }
  const before = zones.length
    ? row.line.zones.map((zone) => (zone.previous_reading === null ? '—' : quantity(zone.previous_reading))).join(' / ')
    : row.line.previous_reading === null
      ? '—'
      : quantity(row.line.previous_reading)
  return paid + t('utilityBill.was', { value: before }) + ` · ${price}`
}

/**
 * What the typed readings come to: «расход 125 + 70 кВт·ч», or the formula
 * with the numbers in it «(209 − 205) × 3,5 × 10 = 140».
 */
export function lineCalc(row: BillRow, t: T): string | null {
  const kind = row.line.item.kind
  const readings = typedReadings(row)
  if (!('metered' in kind) || !readings || readInCard(row)) return null
  const used = readings.map((part) => Math.max(0, Number(part.current) - Number(part.previous)))
  const { unit, tariff, formula } = kind.metered
  if (formula && readings.length === 1) {
    const filled = fillFormula(prettyFormula(formula), {
      current: quantity(readings[0].current),
      previous: quantity(readings[0].previous),
      used: quantity(used[0]),
      tariff: quantity(tariff),
    })
    return Number(row.amount) > 0 ? `${filled} = ${quantity(row.amount)}` : filled
  }
  return t('utilityBill.used', { sum: used.map(quantity).join(' + '), unit })
}
