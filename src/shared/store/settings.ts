import { defineStore } from 'pinia'

import { call } from '@/shared/api/ipc'

/**
 * All settings, from two different places.
 *
 * - `app` — theme, language, base currency, favourite currencies. Plain JSON in
 *   the app config directory: they follow the *user*, survive opening another
 *   vault, and are readable before any vault is unlocked (which is what lets the
 *   unlock screen render in the right language).
 *   Also the device's own settings (tray, refresh interval, decimals, sync):
 *   they must never travel to another device with the vault.
 * - `vault` — dashboard widgets, budget, notifications. Encrypted inside the
 *   vault, so they are genuinely per-vault and synced between devices.
 *
 * Both halves share one store on purpose. The backend split is a storage detail;
 * making callers import a second store to find out where a setting comes from
 * would only move the decision into every view. The split stays visible in the
 * field and action names (`app` / `vault`).
 */
/** A catalog that came back cached or built-in is asked for again after this long. */
const CATALOG_RETRY_MS = 60_000

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    /** User-scoped preferences, stored outside the vault. */
    app: null as AppSettings | null,
    /** Vault-scoped preferences, stored encrypted inside it. */
    vault: null as Settings | null,
    /** Currency catalog for the pickers; null until `loadCurrencies()` runs. */
    currencies: null as SupportedCurrencies | null,
    /** `builtin` | `cached` | null — see `CurrencyCatalogOutput.notice`. */
    currenciesNotice: null as string | null,
    currenciesLoading: false,
    /** When the catalog last arrived, ms; a degraded one is asked for again after a minute. */
    currenciesFetchedAt: 0,
    /** Whether the app-level read has landed at least once. */
    appLoaded: false,
    /** Ask for consent to look hashes up (plan §8.2); the shell shows the dialog. */
    enrichmentConsentOpen: false,
  }),

  getters: {
    // -- app-scoped: the values views actually read --

    theme: (state): Theme => state.app?.theme ?? 'auto',
    locale: (state): Locale => state.app?.locale ?? 'auto',
    /** Lowercase ISO code; always non-empty so labels never render blank. */
    baseCurrency: (state): string => state.app?.base_currency || 'usd',
    /** A fresh install: the first create or open of a vault asks for the base currency. */
    needsBaseCurrency: (state): boolean => state.app !== null && !state.app.base_currency_chosen,
    favoriteCurrencies: (state): string[] => state.app?.favorite_currencies ?? [],
    /** Every code either picker may show, sorted and deduplicated. */
    allCurrencies: (state): string[] => {
      const catalog = state.currencies
      if (!catalog) return []
      return [...new Set([...catalog.fiat, ...catalog.crypto])].sort()
    },
    /** Fiat codes, lowercase; kept apart from crypto so pickers can group and filter. */
    fiatCurrencies: (state): string[] => [...(state.currencies?.fiat ?? [])].sort(),
    cryptoCurrencies: (state): string[] => {
      const fiat = new Set(state.currencies?.fiat ?? [])
      return [...(state.currencies?.crypto ?? [])].filter((code) => !fiat.has(code)).sort()
    },

    // -- vault-scoped --

    /** Decimal places used to render balances: a device setting, the vault's old copy until taken over. */
    displayDecimals: (state): number => state.app?.display_decimals ?? state.vault?.display_decimals ?? 8,
    dashboardWidgets: (state): DashboardWidgetCfg[] => state.vault?.dashboard_widgets ?? [],
    /** The vault keeps crypto: crypto wallets, exchanges and coins are shown. Unknown = yes. */
    cryptoEnabled: (state): boolean => state.vault?.crypto !== false,
  },

  actions: {
    async loadApp() {
      this.app = await call<AppSettings>('get_app_settings')
      this.appLoaded = true
    },

    async loadVault() {
      this.vault = await call<Settings>('get_settings')
    },

    /**
     * Fetch the currency catalog from the public providers.
     *
     * The backend degrades on its own (network → last cached fetch → built-in
     * list), so this never rejects over an offline machine; `currenciesNotice`
     * says which rung was used. A degraded list (`notice` set) is asked for
     * again once it is older than a minute, or at once with `force`.
     */
    async loadCurrencies(force = false) {
      if (this.currenciesLoading) return
      const stale = this.currenciesNotice !== null && Date.now() - this.currenciesFetchedAt > CATALOG_RETRY_MS
      if (this.currencies && !force && !stale) return
      this.currenciesLoading = true
      try {
        const result = await call<CurrencyCatalogOutput>('get_supported_currencies')
        this.currencies = result.catalog
        this.currenciesNotice = result.notice
        this.currenciesFetchedAt = Date.now()
      } finally {
        this.currenciesLoading = false
      }
    },

    /** Persist a partial update and keep the store in step with the backend. */
    async updateApp(patch: Partial<UpdateAppSettingsInput>) {
      this.app = await call<AppSettings>('update_app_settings', {
        input: {
          theme: patch.theme ?? null,
          locale: patch.locale ?? null,
          base_currency: patch.base_currency ?? null,
          base_currency_chosen: patch.base_currency_chosen ?? null,
          favorite_currencies: patch.favorite_currencies ?? null,
          timezone: patch.timezone ?? null,
          inflation: patch.inflation ?? null,
          enrichment_opt_in: patch.enrichment_opt_in ?? null,
          completed_tours: patch.completed_tours ?? null,
          seen_whats_new: patch.seen_whats_new ?? null,
          device_name: patch.device_name ?? null,
          sync: patch.sync ?? null,
          minimize_to_tray: patch.minimize_to_tray ?? null,
          fx_refresh_interval_sec: patch.fx_refresh_interval_sec ?? null,
          display_decimals: patch.display_decimals ?? null,
          dashboard_today_collapsed: patch.dashboard_today_collapsed ?? null,
        } satisfies UpdateAppSettingsInput,
      })
    },

    /** Register the global «+ Операция» key; `null` turns it off. Rejects with `SHORTCUT_FAILED`. */
    async setQuickAddShortcut(shortcut: string | null) {
      this.app = await call<AppSettings>('set_quick_add_shortcut', { shortcut })
    },

    async updateVault(patch: Partial<Settings>) {
      this.vault = await call<Settings>('update_settings', { input: vaultPatchToInput(patch) })
    },
  },
})

function vaultPatchToInput(patch: Partial<Settings>): UpdateSettingsInput {
  return {
    display_decimals: patch.display_decimals ?? null,
    dashboard_widgets: patch.dashboard_widgets ?? null,
    dashboard_period: patch.dashboard_period ?? null,
    fx_refresh_interval_sec: patch.fx_refresh_interval_sec ?? null,
    minimize_to_tray: patch.minimize_to_tray ?? null,
    savings_rate: patch.savings_rate ?? null,
    daily_budget_mode: patch.daily_budget_mode ?? null,
    budget_currency: patch.budget_currency ?? null,
    notifications: patch.notifications ?? null,
    onboarding_skipped: patch.onboarding_skipped ?? null,
    onboarding_checklist_hidden: patch.onboarding_checklist_hidden ?? null,
    manual_income: patch.manual_income ?? null,
    manual_expenses: patch.manual_expenses ?? null,
    crypto: patch.crypto ?? null,
  }
}
