import { MAX_DATE_YEAR, MIN_DATE_YEAR, isDateWithinBounds } from '@/shared/lib/limits'
import { type MonthRange, stepRange } from '@/shared/lib/period'

/** True when the year may be shown by the grid: the years the dates allow. */
export const yearInBounds = (year: number) => year >= MIN_DATE_YEAR && year <= MAX_DATE_YEAR

/** True when a `YYYY-MM` month lies inside the date year range. */
export const monthInBounds = (key: string) => isDateWithinBounds(key)

/** True when the arrows may step the period: both ends of the next one stay inside the bounds. */
export function canStepRange(range: MonthRange, direction: -1 | 1): boolean {
  const next = stepRange(range, direction)
  return monthInBounds(next.from) && monthInBounds(next.to)
}
