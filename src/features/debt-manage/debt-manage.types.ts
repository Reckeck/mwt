export interface DebtPanelProps {
  contact: Contact
  /** The contact's open debts per asset (`get_contact_balances`). */
  balances: ContactBalance[]
}
