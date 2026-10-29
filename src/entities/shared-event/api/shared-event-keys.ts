/**
 * Query keys of the shared events. Everything sits under `all`, so one
 * invalidation after any change (a purchase, a return, a write-off) refreshes
 * the list, the open event, the contact blocks and the chat text.
 */
export const sharedEventKeys = {
  all: ['shared-events'] as const,
  list: (includeArchived: boolean) => [...sharedEventKeys.all, 'list', includeArchived] as const,
  detail: (id: string) => [...sharedEventKeys.all, 'detail', id] as const,
  contact: (contactId: string) => [...sharedEventKeys.all, 'contact', contactId] as const,
  shareText: (id: string, locale: string) => [...sharedEventKeys.all, 'share-text', id, locale] as const,
}
