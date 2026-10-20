import { PLANNING_KEYS } from '@/shared/lib/planning'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/** The ledger collections: everything on screen is derived from them. */
const LEDGER_COLLECTIONS = new Set(['transactions', 'stores'])

/** What each of the other collections shows, as query keys. */
const KEYS_OF: Record<string, ReadonlyArray<readonly unknown[]>> = {
  categories: [['categories']],
  contacts: [['contacts']],
  budgets: PLANNING_KEYS,
  goals: PLANNING_KEYS,
  reminders: [['reminders'], ['upcoming']],
  automations: [['automations'], ['automation-runs'], ['upcoming']],
  workflows: [
    WORKFLOW_KEYS.catalog,
    WORKFLOW_KEYS.list,
    ['workflow'],
    WORKFLOW_KEYS.runs,
    WORKFLOW_KEYS.queue,
    WORKFLOW_KEYS.waits,
    WORKFLOW_KEYS.vars,
  ],
  notification_log: [['notifications']],
}

/**
 * The query keys a synced batch makes stale, or `'all'` when it touched the
 * ledger (or something unknown): then every screen re-reads, as before.
 * A batch of settings or reminders no longer reloads the whole app.
 */
export function keysOfBatch(summary: SyncSummary): ReadonlyArray<readonly unknown[]> | 'all' {
  const keys: Array<readonly unknown[]> = []
  for (const { collection } of summary.collections) {
    if (LEDGER_COLLECTIONS.has(collection)) return 'all'
    if (collection === 'settings') continue
    const own = KEYS_OF[collection]
    if (!own) return 'all'
    keys.push(...own)
  }
  return keys
}

/** Whether the batch changed the vault's settings (kept in the settings store, not in a query). */
export const touchesSettings = (summary: SyncSummary) =>
  summary.collections.some(({ collection }) => collection === 'settings')
