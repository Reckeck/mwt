/** A transfer or a debt operation started from a contact, optionally to one of its requisites. */
export interface ContactTransfer {
  effect: DebtEffect | null
  accountId: string | null
}
