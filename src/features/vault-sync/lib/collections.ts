/** Collections a sync names in its list of changes, in the order they read best. */
export const SYNC_COLLECTIONS = [
  'transactions',
  'stores',
  'categories',
  'contacts',
  'budgets',
  'envelopes',
  'goals',
  'reminders',
  'debt_terms',
  'shared_events',
  'valuations',
  'manual_prices',
  'tx_properties',
  'enrichments',
  'emergency_fund',
  'health_last',
  'asset_classes',
  'asset_class_of',
  'inflation_points',
  'key_rates',
  'calc_scenarios',
  'utility_items',
  'meter_readings',
  'price_alerts',
  'categorization_rules',
  'workflows',
  'workflow_vars',
  'workflow_queue',
  'workflow_state',
  'workflow_waits',
  'workflow_runs',
  'notification_log',
  'pending_imports',
  'import_runs',
  'discarded_imports',
  'exchanges',
  'data_sources',
  'asset_price_sources',
  'deposit_rate_sources',
  'attachments',
  'attachment_keys',
  'price_cache',
  'fx_cache',
  'settings',
] as const

/** i18n key of a collection's name; only a name from a future version shows as `Other`. */
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
