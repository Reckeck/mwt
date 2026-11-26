import { BUDGET_WARN_PERCENT } from '@/features/budget-edit'
import type { ProgressTone } from '@/shared/ui/progress-bar'

export interface BudgetBar {
  /** Fill of the track, 0..100. */
  value: number
  tone: ProgressTone
  /** Where the limit sits on the track once spending passed it; none while under. */
  marker?: number
}

/**
 * The bar of a Budget row. Under the limit the fill is the used share; past it the fill is full and
 * a tick shows where the limit was. Amber from the warning share when the limit warns, red past the limit.
 */
export function budgetBar(usedRatio: number, warn: boolean): BudgetBar {
  const percent = Math.max(0, Math.round((Number.isFinite(usedRatio) ? usedRatio : 0) * 100))
  if (percent > 100) return { value: 100, tone: 'destructive', marker: (100 / percent) * 100 }
  if (percent === 100) return { value: 100, tone: 'destructive' }
  return { value: percent, tone: warn && percent >= BUDGET_WARN_PERCENT ? 'warning' : 'primary' }
}

/** Days left in the period, at least one: the divisor of «left per day». */
export function daysLeft(periodEnd: string, now = Date.now()): number {
  return Math.max(1, Math.ceil((Date.parse(periodEnd) - now) / 86_400_000))
}
