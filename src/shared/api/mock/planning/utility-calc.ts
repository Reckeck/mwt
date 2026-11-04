/** Metered bills: trends, payments and the demo flat. */

import { money } from './budget-calc'
import { mockContext, readings, utilities, uuid } from './state'
import { meterBill } from './utility-formula'

/** One zone's months, like `zone_months` in Rust: a month runs from the last
 * reading before it, the first month of a meter from its own first reading. */
function zoneMonths(own: MeterReading[]) {
  const periods = [...new Set(own.map((entry) => entry.period))]
  return new Map(
    periods.map((period, index) => {
      const group = own.filter((entry) => entry.period === period)
      const before = index ? own.filter((entry) => entry.period === periods[index - 1]).at(-1) : undefined
      return [
        period,
        { group, last: group[group.length - 1], start: before ?? (group.length > 1 ? group[0] : undefined) },
      ]
    }),
  )
}

const sameZone = (left: string | null, right: string | null) =>
  (left ?? '').trim().toLowerCase() === (right ?? '').trim().toLowerCase()

/** Ids of readings lower than the previous one of the same zone (`out_of_order_ids` in Rust). */
function outOfOrderIds(own: MeterReading[]): string[] {
  const last = new Map<string, number>()
  const out: string[] = []
  for (const entry of own) {
    const zone = (entry.zone ?? '').trim().toLowerCase()
    const before = last.get(zone)
    if (before !== undefined && Number(entry.value) < before) out.push(entry.id)
    last.set(zone, Number(entry.value))
  }
  return out
}

/** Bills by month: a meter by its formula or consumption × tariff, per zone when it has them. */
export function utilityTrend(item: UtilityItem): UtilityTrend {
  const sortKey = (entry: MeterReading) => `${entry.period}-${String(entry.day ?? 99).padStart(2, '0')}`
  const own = readings
    .filter((entry) => entry.item_id === item.id)
    .sort((left, right) => sortKey(left).localeCompare(sortKey(right)))
  const paid = paymentsOf(item)
  const zones: (string | null)[] =
    'metered' in item.kind && item.kind.metered.zones.length ? item.kind.metered.zones : [null]
  const perZone = zones.map((zone) => ({ zone, months: zoneMonths(own.filter((entry) => sameZone(entry.zone, zone))) }))
  const meterMonths = [...(perZone[0]?.months.keys() ?? [])].flatMap((period): UtilityMonth[] => {
    const parts = perZone.map(({ zone, months }) => ({ zone, month: months.get(period) }))
    if (parts.some((part) => !part.month)) return []
    const started = parts.every((part) => part.month?.start)
    const end = parts.reduce((sum, part) => sum + Number(part.month?.last.value), 0)
    const start = parts.reduce((sum, part) => sum + Number(part.month?.start?.value ?? 0), 0)
    const typed = parts.map((part) => ({
      zone: part.zone,
      previous: part.month?.start?.value ?? '0',
      current: part.month?.last.value ?? '0',
    }))
    return [
      {
        period,
        reading: String(end),
        consumption: started ? String(end - start) : null,
        amount: money(started ? meterBill(item, typed) : 0),
        reading_id: parts[0].month?.last.id ?? null,
        payments: paid.get(period) ?? [],
        readings: own.filter((entry) => entry.period === period),
        out_of_order: outOfOrderIds(own).filter((id) =>
          own.some((entry) => entry.id === id && entry.period === period),
        ),
        zones: parts.flatMap((part) =>
          part.zone === null
            ? []
            : [
                {
                  zone: part.zone,
                  reading: part.month?.last.value ?? '0',
                  consumption: part.month?.start
                    ? String(Number(part.month.last.value) - Number(part.month.start.value))
                    : null,
                },
              ],
        ),
      },
    ]
  })
  const months: UtilityMonth[] =
    'metered' in item.kind
      ? meterMonths
      : [...paid.entries()]
          .sort(([left], [right]) => left.localeCompare(right))
          .map(([period, payments]) => ({
            period,
            reading: null,
            consumption: null,
            amount: money(payments.reduce((sum, tx) => sum + ('payment' in tx ? Number(tx.payment.amount) : 0), 0)),
            reading_id: null,
            payments,
            readings: [],
            zones: [],
            out_of_order: [],
          }))
  const billed = months.map((month) => Number(month.amount)).filter((value) => value > 0)
  const average = billed.length ? billed.reduce((sum, value) => sum + value, 0) / billed.length : null
  const fixed =
    'static' in item.kind ? item.kind.static.amount : 'housing' in item.kind ? item.kind.housing.amount : '0'
  return {
    item_id: item.id,
    currency: item.currency,
    months,
    average: average === null ? null : money(average),
    delta_pct: null,
    forecast: average === null ? null : money(average),
    anomaly: false,
    monthly: billed.length ? money(billed[billed.length - 1]) : fixed,
  }
}

/** The demo flat of Wallets-2 · 10: a day/night meter not read this month, two
 * water meters (one by its own formula), the internet, the rent an automation pays. */
export function seedUtilityDemo(propertyId: string) {
  if (utilities.length) return
  const base = { active: true, currency: 'rub', property_id: propertyId, store_id: null, category: 'utilities' }
  const metered = (
    name: string,
    unit: string,
    tariff: string,
    formula: string | null,
    zones: string[],
  ): UtilityItem => ({
    id: uuid(),
    name,
    kind: { metered: { unit, tariff, formula, zones } },
    ...base,
  })
  const power = metered('Электричество', 'кВт·ч', '6.5', null, ['день', 'ночь'])
  const cold = metered('Холодная вода', 'м³', '49', null, [])
  const hot = metered('Горячая вода', 'м³', '35', 'расход * тариф', [])
  const net: UtilityItem = { id: uuid(), name: 'Интернет', kind: { static: { amount: '700' } }, ...base }
  const rent: UtilityItem = {
    id: uuid(),
    name: 'Аренда',
    kind: { housing: { amount: '35000', deposit: null, due_day: 10, automation_id: uuid() } },
    ...base,
    category: 'housing',
  }
  utilities.push(power, cold, hot, net, rent)
  const now = new Date()
  const read = (item: UtilityItem, value: number, period: string, zone: string | null = null) =>
    readings.push({
      id: uuid(),
      item_id: item.id,
      value: String(value),
      period,
      recorded_at: `${period}-25T12:00:00Z`,
      day: null,
      zone,
    })
  // Six months up to the last one; the current month has the water read, the power not yet.
  for (let back = 6; back >= 0; back -= 1) {
    const month = new Date(now.getFullYear(), now.getMonth() - back, 1)
    const period = `${month.getFullYear()}-${String(month.getMonth() + 1).padStart(2, '0')}`
    if (back) {
      read(power, 13395 - (back - 1) * 140 + (back % 2) * 6, period, 'день')
      read(power, 5120 - (back - 1) * 55, period, 'ночь')
    }
    read(cold, 418 - back * 6, period)
    read(hot, 209 - back * 4, period)
  }
}

/** `UtilityBill`'s totals, like `utility_bill` in Rust: the amounts and the unread
 * meters' estimates, without the rent an automation pays; the six months' average. */
export function billTotals(lines: UtilityBillLine[], period: string) {
  const counted = lines.filter((line) => !('housing' in line.item.kind && line.item.kind.housing.automation_id))
  const total = counted.reduce((sum, line) => sum + Number(line.amount ?? line.estimate ?? 0), 0)
  const unread = counted.filter((line) => 'metered' in line.item.kind && line.amount === null).length
  const byMonth = new Map<string, number>()
  for (const line of counted)
    for (const month of utilityTrend(line.item).months.filter((entry) => entry.period < period))
      byMonth.set(month.period, (byMonth.get(month.period) ?? 0) + Number(month.amount))
  const recent = [...byMonth.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([, value]) => value)
    .filter((value) => value > 0)
    .slice(-6)
  const average = recent.length ? recent.reduce((sum, value) => sum + value, 0) / recent.length : null
  return {
    total: money(total),
    estimated: unread > 0,
    unread,
    average: average === null ? null : money(average),
    delta_pct: average && total ? String(Math.round(((total - average) / average) * 100)) : null,
  }
}

/** Payments linked to a bill, by `YYYY-MM`. */
export function paymentsOf(item: UtilityItem): Map<string, Transaction[]> {
  const out = new Map<string, Transaction[]>()
  for (const tx of mockContext.value?.transactions ?? []) {
    if (!('payment' in tx) || tx.payment.meta.utility_item_id !== item.id) continue
    const period = tx.payment.meta.timestamp.slice(0, 7)
    out.set(period, [...(out.get(period) ?? []), tx])
  }
  return out
}
