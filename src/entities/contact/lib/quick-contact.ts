/**
 * A «quick» contact is one made on the spot, by name only: it has no
 * requisites yet. Computed, never stored, so the vault schema stays as it is.
 */
export const isQuickContact = (contact: Contact): boolean => contact.accounts.length === 0

/** Live quick contacts: the `To fill in` filter and the counter. */
export const quickContacts = (contacts: Contact[]): Contact[] =>
  contacts.filter((contact) => !contact.archived && isQuickContact(contact))
