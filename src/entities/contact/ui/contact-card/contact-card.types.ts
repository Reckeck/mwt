export interface ContactCardProps {
  contact: Contact
  /** Personal debts with this contact, per asset. */
  balances: ContactBalance[]
  /** Money moved with this contact. */
  turnover: ContactTurnover | null
}
