import { formatMoney, formatMonthKey, formatQuantity } from '@/shared/lib/format'

type T = (key: string, values?: Record<string, unknown>) => string

/** One month of the item's history (Wallets-2 · 11–12), already worded. */
export interface UtilityHistoryRow {
  period: string
  month: string
  /** «13 395 / 5 120», «209», «—». */
  reading: string
  /** «120 / 75», «4», «—». */
  consumption: string
  amount: string
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
  const money = (value: string) => formatMoney(value, { currency: item.currency, decimals: 2 })
  const month = (key: string) => formatMonthKey(key, { month: 'long' }).slice(0, 3)
  const months = [...trend.months].reverse().slice(0, 4)
  const rows: UtilityHistoryRow[] = months.map((entry, index) => ({
    period: entry.period,
    month: month(entry.period),
    reading: entry.zones.length ? entry.zones.map((zone) => number(zone.reading)).join(' / ') : number(entry.reading),
    consumption: entry.zones.length
      ? entry.zones.map((zone) => number(zone.consumption)).join(' / ')
      : number(entry.consumption),
    amount: money(entry.amount),
    waiting: false,
    anomaly: index === 0 && trend.anomaly,
  }))
  if (rows[0]?.period === period) return rows
  const empty = {
    period,
    month: month(period),
    reading: '—',
    consumption: '—',
    amount: '—',
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

/**
 * The formula with the typed numbers in place of its words:
 * «(показание − предыдущее) × 3,5» → «(209 − 205) × 3,5». Display only; the
 * amount itself comes from `preview_utility_formula`.
 */
export function fillFormula(
  formula: string,
  values: { current: string; previous: string; used: string; tariff: string },
) {
  const words = new Map([
    ['показание', values.current],
    ['предыдущее', values.previous],
    ['расход', values.used],
    ['тариф', values.tariff],
  ])
  return formula.replace(/[\p{L}_][\p{L}\p{N}_]*/gu, (word) => words.get(word.toLowerCase().replace(/ё/g, 'е')) ?? word)
}

/** The formula as typed, with × and − as the canvas prints them. */
export const prettyFormula = (formula: string) =>
  formula
    .replace(/\*/g, '×')
    .replace(/(\s)-(\s)/g, '$1−$2')
    .replace(/(\d)\.(\d)/g, '$1,$2')
