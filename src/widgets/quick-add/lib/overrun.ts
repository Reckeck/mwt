import { decimalCmp, decimalSub } from '@/shared/lib/decimal'

export interface Overrun {
  /** The budget it breaks. */
  id: string
  /** How far over the limit the expense takes the period, in the budget's currency. */
  over: string
  currency: string
}

/** The active budgets the expense counts against: the total, its category, or the wallet's store budget. */
export function matchingBudgets(statuses: BudgetStatus[], category: string, storeId: string): BudgetStatus[] {
  return statuses.filter(({ budget }) => {
    if (!budget.active) return false
    const { target } = budget
    if (target === 'total') return true
    if ('category' in target) return target.category === category
    return target.store.store_ids.includes(storeId)
  })
}

/**
 * How far over this budget the expense goes, `converted` being the amount in the budget's
 * currency (`null` when no rate is known); `null` when nothing breaks.
 */
export function overrunOf(status: BudgetStatus, converted: string | null): Overrun | null {
  if (converted === null || decimalCmp(converted, '0') !== 1) return null
  const over = decimalSub(converted, status.remaining)
  return over !== null && decimalCmp(over, '0') === 1
    ? { id: status.budget.id, over, currency: status.budget.currency }
    : null
}
