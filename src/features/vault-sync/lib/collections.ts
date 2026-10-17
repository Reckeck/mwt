/** Collections a sync names in its list of changes, in the order they read best. */
export const SYNC_COLLECTIONS = [
  'transactions',
  'stores',
  'categories',
  'contacts',
  'budgets',
  'goals',
  'reminders',
  'automations',
  'workflows',
  'notification_log',
  'settings',
] as const

/** i18n key of a collection's name; an unknown one shows as «Другое». */
export function collectionKey(name: string): string {
  return (SYNC_COLLECTIONS as readonly string[]).includes(name) ? `sync.coll.${name}` : 'sync.coll.other'
}

/** Changes sorted so the user's money comes first. */
export function sortedChanges(summary: SyncSummary): CollectionChanges[] {
  const rank = (name: string) => {
    const index = (SYNC_COLLECTIONS as readonly string[]).indexOf(name)
    return index === -1 ? SYNC_COLLECTIONS.length : index
  }
  return [...summary.collections].sort((left, right) => rank(left.collection) - rank(right.collection))
}
