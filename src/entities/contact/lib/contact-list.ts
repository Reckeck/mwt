/** Which contacts the list shows: everyone, those who owe me, those I owe. */
export type ContactDebtFilter = 'all' | 'owe_me' | 'i_owe'

export interface ContactNet {
  symbol: string
  /** Positive: they owe me; negative: I owe them. */
  net: number
}

export interface ContactSection {
  key: string
  title: string
  entries: Contact[]
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
 * a card number finds its owner — and the debt filter; archived ones last.
 */
export function filterContacts(
  contacts: Contact[],
  balances: ContactBalance[],
  search: string,
  filter: ContactDebtFilter,
): Contact[] {
  const needle = search.trim().toLowerCase()
  return contacts
    .filter(
      (contact) =>
        !needle ||
        contact.name.toLowerCase().includes(needle) ||
        contact.group.toLowerCase().includes(needle) ||
        contact.accounts.some((account) => account.value.toLowerCase().includes(needle)),
    )
    .filter((contact) => {
      if (filter === 'all') return true
      const nets = contactNets(balances, contact.id)
      return filter === 'owe_me' ? nets.some((row) => row.net > 0) : nets.some((row) => row.net < 0)
    })
    .sort((left, right) => Number(left.archived) - Number(right.archived) || left.name.localeCompare(right.name))
}

/**
 * The list in sections: the user's own groups by name first, then the
 * contacts without one by kind (people, companies, exchangers).
 */
export function contactSections(list: Contact[], kindTitle: (kind: ContactKind) => string): ContactSection[] {
  const named = new Map<string, Contact[]>()
  const byKind = new Map<ContactKind, Contact[]>()
  for (const contact of list) {
    const group = contact.group.trim()
    if (group) named.set(group, [...(named.get(group) ?? []), contact])
    else byKind.set(contact.kind, [...(byKind.get(contact.kind) ?? []), contact])
  }
  return [
    ...[...named.entries()]
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([title, entries]) => ({ key: `group:${title}`, title, entries })),
    ...(['person', 'company', 'exchanger'] as ContactKind[])
      .filter((kind) => byKind.has(kind))
      .map((kind) => ({ key: `kind:${kind}`, title: kindTitle(kind), entries: byKind.get(kind) ?? [] })),
  ]
}
