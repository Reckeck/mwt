/** Periods of whole months (`YYYY-MM`): quick presets, stepping, the month grid. */

export type PeriodPreset = 'thisMonth' | 'lastMonth' | 'last3' | 'last6' | 'last12' | 'thisYear' | 'all'

/** The quick periods, in the picker's order. */
export const PERIOD_PRESETS: PeriodPreset[] = ['thisMonth', 'lastMonth', 'last3', 'last6', 'last12', 'thisYear', 'all']

/** A period: both ends set, `from <= to`. */
export interface MonthRange {
  from: string
  to: string
}

/** «All time» starts here: the reports cap a period at ten years. */
const ALL_YEARS = 10

/** The key of a month counted from `year`/`month` (0-based, may overflow). */
export function monthKey(year: number, month: number): string {
  const date = new Date(year, month, 1, 12)
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`
}

export function currentMonth(now = new Date()): string {
  return monthKey(now.getFullYear(), now.getMonth())
}

/** The local day of `date` as `YYYY-MM-DD`, the form dates take in DTOs. */
export function dayKey(date = new Date()): string {
  return `${currentMonth(date)}-${String(date.getDate()).padStart(2, '0')}`
}

/** `key` moved by `delta` months. */
export function shiftMonth(key: string, delta: number): string {
  const [year, month] = key.split('-').map(Number)
  return monthKey(year, month - 1 + delta)
}

/** How many months a range covers, both ends included. */
export function monthsIn(range: MonthRange): number {
  const [fromYear, fromMonth] = range.from.split('-').map(Number)
  const [toYear, toMonth] = range.to.split('-').map(Number)
  return (toYear - fromYear) * 12 + (toMonth - fromMonth) + 1
}

export function presetRange(preset: PeriodPreset, now = new Date()): MonthRange {
  const year = now.getFullYear()
  const month = now.getMonth()
  const to = monthKey(year, month)
  switch (preset) {
    case 'thisMonth':
      return { from: to, to }
    case 'lastMonth':
      return { from: monthKey(year, month - 1), to: monthKey(year, month - 1) }
    case 'last3':
      return { from: monthKey(year, month - 2), to }
    case 'last6':
      return { from: monthKey(year, month - 5), to }
    case 'last12':
      return { from: monthKey(year, month - 11), to }
    case 'thisYear':
      return { from: monthKey(year, 0), to }
    case 'all':
      return { from: monthKey(year - ALL_YEARS, month + 1), to }
  }
}

/** The preset a range matches, or `''` for a custom one. */
export function presetOf(range: MonthRange, now = new Date()): PeriodPreset | '' {
  return (
    PERIOD_PRESETS.find((preset) => {
      const entry = presetRange(preset, now)
      return entry.from === range.from && entry.to === range.to
    }) ?? ''
  )
}

/** The same length of period, `direction` periods earlier (−1) or later (+1). */
export function stepRange(range: MonthRange, direction: -1 | 1): MonthRange {
  const length = monthsIn(range)
  return { from: shiftMonth(range.from, direction * length), to: shiftMonth(range.to, direction * length) }
}

/**
 * A click on a month of the grid: that month alone, or, with `extend` (Shift),
 * from the period's start to it.
 */
export function pickMonth(range: MonthRange, key: string, extend: boolean): MonthRange {
  if (!extend) return { from: key, to: key }
  return key < range.from ? { from: key, to: range.to } : { from: range.from, to: key }
}

/** What an empty `from`/`to` of a report means: eleven months back, and this month. */
export function defaultRange(now = new Date()): MonthRange {
  return presetRange('last12', now)
}

/** First and last day of a range, `YYYY-MM-DD`. */
export function rangeDays(range: MonthRange): { first: string; last: string } {
  const [year, month] = range.to.split('-').map(Number)
  const end = new Date(year, month, 0, 12).getDate()
  return { first: `${range.from}-01`, last: `${range.to}-${String(end).padStart(2, '0')}` }
}
