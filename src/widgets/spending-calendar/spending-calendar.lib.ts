import { HEAT_LEVELS } from './spending-calendar.constants'

/**
 * Heat step of a day's spending against the month's heaviest day: 0 for a quiet day, 1…4 in
 * equal shares of the maximum. A day that spent anything is never step 0.
 */
export function heatLevel(spent: number, max: number): number {
  if (!(spent > 0) || !(max > 0)) return 0
  const top = HEAT_LEVELS - 1
  return Math.min(top, Math.max(1, Math.ceil((spent / max) * top)))
}
