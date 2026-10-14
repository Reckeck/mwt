/** The options a kind offers: category groups and debt effects. */

import type { TransactionFormState } from './form-state'

/** Which groups a kind's categories come from (mirrors V12 in Rust). */
export function categoryGroupsFor(kind: TxKind): CategoryGroup[] {
  if (kind === 'topup') return ['income', 'service']
  if (kind === 'payment') return ['fixed', 'daily', 'savings', 'service']
  return ['income', 'fixed', 'daily', 'savings', 'service']
}

/**
 * The debt effect a transfer can carry, from its direction: money leaving my
 * wallet lends or repays, money arriving borrows or collects.
 */
export function debtChoices(form: TransactionFormState): DebtEffect[] {
  const fromContact = form.from !== null && 'contact' in form.from
  const toContact = form.to !== null && 'contact' in form.to
  if (toContact && form.from && 'store' in form.from) return ['lend', 'repay']
  if (fromContact && form.to && 'store' in form.to) return ['borrow', 'collect']
  return []
}
