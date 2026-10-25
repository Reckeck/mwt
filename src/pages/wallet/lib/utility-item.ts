import { formatMonthKey, formatQuantity } from '@/shared/lib/format'
import type { SumPart } from '@/shared/lib/money-parts'

type T = (key: string, values?: Record<string, unknown>) => string

/** One month of the item's history (Wallets-2 · 11–12), already worded. */
export interface UtilityHistoryRow {
  period: string
  month: string
  /** «13 395 / 5 120», «209», «—». */
  reading: string
  /** «120 / 75», «4», «—». */
  consumption: string
  /** The month's sum; `null` while there is none yet («—»). */
  amount: SumPart | null
  /** This month is not read yet: «ждёт». */
  waiting: boolean
  /** The last bill is well above the average: «необычно много». */
  anomaly: boolean
}

const number = (value: string | null | undefined) =>
  value === null || value === undefined ? '—' : formatQuantity(value, 2)

/**
 * The item's months newest first; this month leads with «ждёт» while it has no
 * reading. Only the last month can carry the anomaly: core flags no other.
 */
export function historyRows(item: UtilityItem, trend: UtilityTrend, period: string): UtilityHistoryRow[] {
  const month = (key: string) => formatMonthKey(key, { month: 'long' }).slice(0, 3)
  const months = [...trend.months].reverse().slice(0, 4)
  const rows: UtilityHistoryRow[] = months.map((entry, index) => ({
    period: entry.period,
    month: month(entry.period),
    reading: entry.zones.length ? entry.zones.map((zone) => number(zone.reading)).join(' / ') : number(entry.reading),
    consumption: entry.zones.length
      ? entry.zones.map((zone) => number(zone.consumption)).join(' / ')
      : number(entry.consumption),
    amount: { amount: entry.amount, currency: item.currency, decimals: 2, at: entry.period },
    waiting: false,
    anomaly: index === 0 && trend.anomaly,
  }))
  if (rows[0]?.period === period) return rows
  const empty = {
    period,
    month: month(period),
    reading: '—',
    consumption: '—',
    amount: null,
    waiting: true,
    anomaly: false,
  }
  return [empty, ...rows.slice(0, 3)]
}

/** The kind under the name: «по счётчику · две зоны · кВт·ч», «… · одно показание · своя формула». */
export function itemSubtitle(item: UtilityItem, t: T): string {
  if (!('metered' in item.kind)) return t('utilityTab.fixed')
  const { unit, zones, formula } = item.kind.metered
  const count = zones.length
    ? t(`utilityItem.zones${Math.min(Math.max(zones.length, 2), 4)}`)
    : t('utilityItem.oneReading')
  return [t('utilityTab.metered'), count, formula ? t('utilityTab.ownFormula') : unit].join(' · ')
}
