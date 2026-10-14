/** Metered bills: trends, payments and the demo flat. */

import { money } from './budget-calc'
import { mockContext, readings, utilities, uuid } from './state'

/** Bills by month of a metered item: consumption × tariff. */
export function utilityTrend(item: UtilityItem): UtilityTrend {
  const sortKey = (entry: MeterReading) => `${entry.period}-${String(entry.day ?? 99).padStart(2, '0')}`
  const own = readings
    .filter((entry) => entry.item_id === item.id)
    .sort((left, right) => sortKey(left).localeCompare(sortKey(right)))
  const tariff = 'metered' in item.kind ? Number(item.kind.metered.tariff) : 0
  const paid = paymentsOf(item)
  // Like `monthly_readings` in Rust: a month runs from the last reading
  // before it, the first month of a meter from its own first reading.
  const periods = [...new Set(own.map((entry) => entry.period))]
  const months: UtilityMonth[] =
    'metered' in item.kind
      ? periods.map((period, index) => {
          const group = own.filter((entry) => entry.period === period)
          const last = group[group.length - 1]
          const before = index ? own.filter((entry) => entry.period === periods[index - 1]).at(-1) : undefined
          const start = before ?? (group.length > 1 ? group[0] : undefined)
          const consumption = start ? Number(last.value) - Number(start.value) : null
          return {
            period,
            reading: last.value,
            consumption: consumption === null ? null : String(consumption),
            amount: money((consumption ?? 0) * tariff),
            reading_id: last.id,
            payments: paid.get(period) ?? [],
            readings: group,
          }
        })
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
    kind: { metered: { unit: 'кВт·ч', tariff: '6.5' } },
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
