export interface ContactNet {
  symbol: string
  /** Positive: they owe me; negative: I owe them. */
  net: number
}

/** Net debt per contact and asset, without the settled ones. */
export function contactNets(balances: ContactBalance[], contactId: string): ContactNet[] {
  return balances
    .filter((row) => row.contact_id === contactId)
    .map((row) => ({ symbol: row.asset.symbol, net: Number(row.receivable) - Number(row.payable) }))
    .filter((row) => row.net !== 0)
}

/**
 * The contacts matching the search — by name, group or any account value, so
 * a card number finds its owner; archived ones last.
 */
export function filterContacts(contacts: Contact[], search: string): Contact[] {
  const needle = search.trim().toLowerCase()
  return contacts
    .filter(
      (contact) =>
        !needle ||
        contact.name.toLowerCase().includes(needle) ||
        contact.group.toLowerCase().includes(needle) ||
        contact.accounts.some((account) =>
          account.value.replace(/\s/g, '').toLowerCase().includes(needle.replace(/\s/g, '')),
        ),
    )
    .sort((left, right) => Number(left.archived) - Number(right.archived) || left.name.localeCompare(right.name))
}
