/** The vault status, its settings and the JSON dump. */

import { mockCategories, mockContacts, mockRules } from './ledger'
import { mockDebtTerms, mockEnvelopes } from './catalog'
import { mockStores, mockTransactions } from './demo'
import { fund } from './planning/emergency'
import { mockPropertyLinks } from './property-links'

/** Reassigned by commands: a holder, since an imported binding cannot be assigned. */
export const mockStatus: { value: VaultStatus } = {
  value: {
    path: '/tmp/mock/vault.wallet',
    exists: false,
    unlocked: false,
    info: null,
    // `?storage=app` shows the Android/iOS screens (vaults in app storage).
    storage: new URLSearchParams(globalThis.location?.search ?? '').get('storage') === 'app' ? 'app' : 'files',
    encrypted: true,
    sandbox: false,
  },
}

/** The vault password of the mock (`null` = a vault without a password); only the password commands change it. */
export const mockPassword: { value: string | null } = { value: 'secret' }

export const mockInfo: VaultInfo = {
  path: '/tmp/mock/vault.wallet',
  doc_version: 2,
  id: crypto.randomUUID(),
  created_at: new Date().toISOString(),
  format: 'json',
  base_currency: 'usd',
  encrypted: true,
}

export const mockSettings: Settings = {
  display_decimals: 8,
  dashboard_widgets: [],
  dashboard_period: 'month',
  onboarding_checklist_hidden: false,
  inflation_mode: {},
  inflation_source: {},
  fx_refresh_interval_sec: 300,
  minimize_to_tray: true,
  savings_rate: '0.10',
  daily_budget_mode: 'month',
  notifications: { system: true, hide_details_when_locked: false, default_time: '10:00', categories: [] },
  budget_currency: '',
  budget_configured: false,
  onboarding_skipped: [],
  manual_income: null,
  manual_expenses: null,
  freedom_rate: null,
  carry_since: null,
  pinned: [],
  categories_rev: 1,
  // The demo holds exchanges and crypto wallets; `?crypto=0` previews a vault without them.
  crypto: new URLSearchParams(globalThis.location?.search ?? '').get('crypto') !== '0',
}

/** Mirrors what `app_settings.json` would hold. */
export const mockAppSettings: AppSettings = {
  theme: 'dark',
  locale: 'auto',
  base_currency: 'usd',
  // A fresh install: the create and open forms ask for the base currency.
  base_currency_chosen: false,
  favorite_currencies: ['rub', 'usdt', 'thb', 'btc'],
  timezone: null,
  inflation: { rub: '8', usd: '3', eur: '2.5', thb: '1' },
  enrichment_opt_in: null,
  quick_add_shortcut: null,
  completed_tours: [],
  seen_whats_new: [],
  device_id: '00000000-0000-4000-8000-000000000001',
  device_name: 'Ноутбук',
  sync: { enabled: false, confirm_first: false, prefer_main: false, background_lock_min: null },
  minimize_to_tray: null,
  fx_refresh_interval_sec: null,
  display_decimals: null,
  dashboard_today_collapsed: false,
  // An install older than the field: «Что нового» shows every entry, as after an update.
  installed_at: null,
}

export const mockCurrencies: SupportedCurrencies = {
  fiat: ['usd', 'eur', 'rub', 'cny', 'gbp', 'jpy', 'chf', 'cad', 'aud', 'thb'],
  // Thousands of synthetic tickers, like the real CoinGecko list, so the picker's virtual scroll gets exercised.
  crypto: [
    'btc',
    'eth',
    'sol',
    'usdt',
    'usdc',
    'ton',
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
    calc_scenarios: [],
    inflation_points: [],
    emergency_fund: fund,
    health_last: null,
    asset_classes: [],
    asset_class_of: [],
    valuations: [],
    tx_properties: [...mockPropertyLinks],
    data_sources: [],
    asset_price_sources: [],
    deposit_rate_sources: [],
    attachments: [],
  }
}
