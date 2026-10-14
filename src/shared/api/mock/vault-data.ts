/** The vault status, its settings and the JSON dump. */

import { mockCategories, mockContacts, mockRules } from './ledger'
import { mockDebtTerms, mockEnvelopes } from './catalog'
import { mockStores, mockTransactions } from './demo'

/** Reassigned by commands: a holder, since an imported binding cannot be assigned. */
export const mockStatus: { value: VaultStatus } = {
  value: {
    path: '/tmp/mock/vault.wallet',
    exists: false,
    unlocked: false,
    info: null,
  },
}

export const mockInfo: VaultInfo = {
  path: '/tmp/mock/vault.wallet',
  doc_version: 2,
  id: crypto.randomUUID(),
  created_at: new Date().toISOString(),
  format: 'json',
  base_currency: 'usd',
}

export const mockSettings: Settings = {
  display_decimals: 8,
  dashboard_widgets: [],
  fx_refresh_interval_sec: 300,
  minimize_to_tray: true,
  savings_rate: '0.10',
  daily_budget_mode: 'month',
  notifications: { system: true, hide_details_when_locked: false, default_time: '10:00', categories: [] },
  budget_currency: '',
}

/** Mirrors what `app_settings.json` would hold. */
export const mockAppSettings: AppSettings = {
  theme: 'dark',
  locale: 'auto',
  base_currency: 'usd',
  favorite_currencies: [],
  timezone: null,
  inflation: { rub: '8', usd: '3', eur: '2.5', thb: '1' },
  enrichment_opt_in: null,
  quick_add_shortcut: null,
  completed_tours: [],
  seen_whats_new: [],
}

export const mockCurrencies: SupportedCurrencies = {
  fiat: ['usd', 'eur', 'rub', 'cny', 'gbp', 'jpy', 'chf', 'cad', 'aud'],
  // Thousands of synthetic tickers, like the real CoinGecko list, so the picker's virtual scroll gets exercised.
  crypto: [
    'btc',
    'eth',
    'sol',
    'usdt',
    'bnb',
    'xrp',
    'doge',
    ...Array.from({ length: 5000 }, (_unused, index) => `tk${index.toString(36)}`),
  ],
}
/** Stand-in for the decrypted `VaultDoc`, mirroring the Rust dump command. */
export function mockVaultDoc(): VaultDoc {
  return {
    version: 2,
    id: mockInfo.id,
    created_at: mockInfo.created_at,
    settings: mockSettings,
    stores: mockStores,
    holdings: [],
    transactions: mockTransactions,
    categories: mockCategories,
    price_cache: [],
    fx_cache: [],
    pending_imports: [],
    contacts: mockContacts,
    automations: [],
    automation_runs: [],
    budgets: [],
    goals: [],
    reminders: [],
    price_alerts: [],
    categorization_rules: mockRules,
    utility_items: [],
    meter_readings: [],
    enrichments: [],
    manual_prices: [],
    envelopes: mockEnvelopes,
    debt_terms: mockDebtTerms,
    key_rates: [],
    notification_log: [],
    workflows: [],
    workflow_runs: [],
    workflow_vars: [],
    workflow_queue: [],
    workflow_state: [],
    workflow_waits: [],
    import_runs: [],
    discarded_imports: [],
  }
}
