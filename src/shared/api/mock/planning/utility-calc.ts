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

/** Two bills of a demo flat with half a year of readings. */
export function seedUtilityDemo(propertyId: string) {
  if (utilities.length) return
  const base = { active: true, currency: 'rub', property_id: propertyId, store_id: null, category: 'utilities' }
  const power: UtilityItem = {
    id: uuid(),
    name: 'Электричество',
    kind: { metered: { unit: 'кВт·ч', tariff: '6.5', formula: null, zones: [] } },
    ...base,
  }
  const net: UtilityItem = { id: uuid(), name: 'Интернет', kind: { static: { amount: '700' } }, ...base }
  utilities.push(power, net)
  const now = new Date()
  let value = 12000
  for (let back = 6; back >= 1; back -= 1) {
    const month = new Date(now.getFullYear(), now.getMonth() - back, 1)
    const period = `${month.getFullYear()}-${String(month.getMonth() + 1).padStart(2, '0')}`
    value += 180 + back * 15
    readings.push({
      id: uuid(),
      item_id: power.id,
      value: String(value),
      period,
      recorded_at: month.toISOString(),
      day: null,
      zone: null,
    })
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
