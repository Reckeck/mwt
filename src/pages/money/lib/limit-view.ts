import { currentMonth } from '@/shared/lib/period'

/** How a category's bar reads against its limit (Money 1 · 5). */
export interface LimitBar {
  /** Share of the bar, percent. */
  percent: number
  tone: 'primary' | 'warning' | 'destructive'
}

/**
 * Spending against a monthly limit within one month: over it — red; ahead of
 * the month (72 % spent by day 22 of 31) — orange; otherwise the usual colour.
 */
export function limitBar(spent: string, limit: string, month: string, now = new Date()): LimitBar {
  const used = (Number(spent) / Math.max(Number(limit), Number.EPSILON)) * 100
  const days = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate()
  const elapsed = month === currentMonth(now) ? (now.getDate() / days) * 100 : 100
  const tone = used > 100 ? 'destructive' : used >= elapsed * 0.9 ? 'warning' : 'primary'
  return { percent: Math.min(100, used), tone }
}

/** A monthly limit compares with the spending of one month in the limit's own currency. */
export function comparable(budget: Budget, from: string, to: string, currency: string): boolean {
  return Boolean(
    budget.period === 'month' && from && from === to && budget.currency.toLowerCase() === currency.toLowerCase(),
  )
}
