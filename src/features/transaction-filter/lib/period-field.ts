import {
  PERIOD_PRESETS,
  currentMonth,
  presetRange,
  rangeDays,
  shiftMonth,
  type PeriodPreset,
} from '@/shared/lib/period'

/** The chips under the month: whole-month presets, then «с … по». */
export const PERIOD_CHIPS = PERIOD_PRESETS.filter((preset) => preset !== 'all')

/** How many months the month list goes back. */
const MONTHS_BACK = 24

/** The months a filter can pick, newest first. */
export function recentMonths(now = new Date()): string[] {
  const latest = currentMonth(now)
  return Array.from({ length: MONTHS_BACK }, (_, index) => shiftMonth(latest, -index))
}

/** The days a preset covers, as the filter holds them. */
export function presetDays(preset: PeriodPreset, now = new Date()): { from: string; to: string } {
  const { first, last } = rangeDays(presetRange(preset, now))
  return { from: first, to: last }
}

/** The days of one month. */
export function monthDays(key: string): { from: string; to: string } {
  const { first, last } = rangeDays({ from: key, to: key })
  return { from: first, to: last }
}

/** The preset the filter's days are, if any. */
export function presetOfDays(from: string | null, to: string | null, now = new Date()): PeriodPreset | '' {
  if (!from || !to) return ''
  return (
    PERIOD_CHIPS.find((preset) => {
      const days = presetDays(preset, now)
      return days.from === from && days.to === to
    }) ?? ''
  )
}

/** The month the filter's days are, if they are exactly one. */
export function monthOfDays(from: string | null, to: string | null): string {
  if (!from || !to || from.slice(0, 7) !== to.slice(0, 7)) return ''
  const days = monthDays(from.slice(0, 7))
  return days.from === from && days.to === to ? from.slice(0, 7) : ''
}
