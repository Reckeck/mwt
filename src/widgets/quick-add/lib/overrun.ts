import { decimalCmp, decimalSub } from '@/shared/lib/decimal'

export interface Overrun {
  /** How far over the limit the expense takes the period. */
  over: string
  currency: string
}

/**
 * The category's limit the expense would break: the amount is more than what is left of it. Only
 * a limit in the wallet's own currency is compared (no rate is guessed); `null` when nothing breaks.
 */
export function overrunOf(
  statuses: BudgetStatus[],
  category: string,
  amount: string,
  currency: string,
): Overrun | null {
  const status = statuses.find(
    (entry) =>
      entry.budget.active &&
      typeof entry.budget.target === 'object' &&
      'category' in entry.budget.target &&
      entry.budget.target.category === category &&
      entry.budget.currency.toLowerCase() === currency.toLowerCase(),
  )
  if (!status || decimalCmp(amount, '0') !== 1) return null
  const over = decimalSub(amount, status.remaining)
  return over !== null && decimalCmp(over, '0') === 1 ? { over, currency: status.budget.currency } : null
}
