import { CAPITAL_HEALTH_KEY } from '@/shared/api/queries'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

type Keys = ReadonlyArray<readonly unknown[]>

/**
 * Collections whose change moves what most screens derive (money, prices, rates, values,
 * classification): the whole app re-reads, as it always did. Listed on purpose, so a new
 * collection is a decision and not a silent fall-through (catalog 126).
 */
export const ALL_COLLECTIONS: ReadonlySet<string> = new Set([
  'stores',
  'transactions',
  'price_cache',
  'fx_cache',
  'manual_prices',
  'asset_price_sources',
  'deposit_rate_sources',
  'key_rates',
  'inflation_points',
  'asset_classes',
  'asset_class_of',
  'valuations',
  'tx_properties',
  'data_sources',
])

/** What a contact, category or reminder shows up in besides its own list. */
const DERIVED: Keys = [['dashboard-today'], ['debt-terms'], ['search-index'], ['shared-events']]

const WORKFLOW_ALL: Keys = [
  ['automations'],
  ['automation-runs'],
  ['upcoming'],
  WORKFLOW_KEYS.catalog,
  WORKFLOW_KEYS.list,
  ['workflow'],
  WORKFLOW_KEYS.runs,
  WORKFLOW_KEYS.queue,
  WORKFLOW_KEYS.waits,
  WORKFLOW_KEYS.vars,
]

/** What each of the other collections shows, as query keys. */
export const KEYS_OF: Record<string, Keys> = {
  categories: [['categories'], ...DERIVED],
  contacts: [['contacts'], ...DERIVED],
  reminders: [['reminders'], ['upcoming'], ...DERIVED],
  budgets: PLANNING_KEYS,
  goals: PLANNING_KEYS,
  envelopes: [['envelopes'], ...PLANNING_KEYS],
  debt_terms: [['debt-terms'], ['contacts'], ['dashboard-today']],
  shared_events: [['shared-events'], ['debt-terms'], ['contacts'], ['dashboard-today']],
  price_alerts: [['price-alerts']],
  categorization_rules: [['categorization-rules']],
  pending_imports: [['pending-imports']],
  discarded_imports: [['pending-imports']],
  enrichments: [['enrichments'], ['transactions'], ['search-index']],
  utility_items: [['utilities'], ['utility-trend'], ['utility-bill'], ['upcoming']],
  meter_readings: [['utilities'], ['utility-trend'], ['utility-bill'], ['upcoming']],
  calc_scenarios: [['calc-scenarios'], ['calc-invest'], ['goals'], ['goal-progress']],
  exchanges: [['bybit-key-status'], ['pending-imports']],
  emergency_fund: [['emergency-fund'], ['net-worth']],
  health_last: [CAPITAL_HEALTH_KEY, ['dashboard-today']],
  attachments: [['attachments']],
  attachment_keys: [['attachments']],
  // Automations are workflows too.
  workflows: WORKFLOW_ALL,
  workflow_vars: WORKFLOW_ALL,
  workflow_queue: WORKFLOW_ALL,
  workflow_state: WORKFLOW_ALL,
  workflow_waits: WORKFLOW_ALL,
  notification_log: [['notifications']],
}

/** The vault's shared settings (budget currency, savings rate…) feed these, though no query holds them. */
const SETTINGS_KEYS: Keys = [
  ['dashboard-today'],
  ['month-summary'],
  ['month-pace'],
  ['daily-budget'],
  ['goals'],
  ['goal-progress'],
]

/**
 * The query keys a synced batch makes stale, or `'all'` when it touched the
 * ledger (or something unknown): then every screen re-reads, as before.
 * A batch of settings or reminders no longer reloads the whole app.
 */
export function keysOfBatch(summary: SyncSummary): Keys | 'all' {
  const keys: Array<readonly unknown[]> = []
  for (const { collection } of summary.collections) {
    if (ALL_COLLECTIONS.has(collection)) return 'all'
    if (collection === 'settings') {
      keys.push(...SETTINGS_KEYS)
      continue
    }
    const own = KEYS_OF[collection]
    if (!own) return 'all'
    keys.push(...own)
  }
  return keys
}

/** Whether the batch changed the vault's settings (kept in the settings store, not in a query). */
export const touchesSettings = (summary: SyncSummary) =>
  summary.collections.some(({ collection }) => collection === 'settings')
