const isContact = (side: AccountRef) => 'contact' in side

/** True when exactly one side of a transfer is a contact: only then can it be a loan. */
export function hasContactSide(from: AccountRef, to: AccountRef): boolean {
  return isContact(from) !== isContact(to)
}

/**
 * The debt effect of a quick transfer marked as a loan, from its sides: money out to a contact
 * lends, money in from a contact borrows. A wallet-to-wallet transfer carries none (the core
 * refuses a debt without a contact side).
 */
export function quickDebtEffect(from: AccountRef, to: AccountRef, asDebt: boolean): DebtEffect | null {
  if (!asDebt || !hasContactSide(from, to)) return null
  return isContact(to) ? 'lend' : 'borrow'
}
