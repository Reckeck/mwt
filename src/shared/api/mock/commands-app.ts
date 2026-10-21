/** Commands: the vault itself, notifications, app settings outside the vault. */

import { txMeta } from '@/shared/lib/transactions'
import { mockCategories, mockRules } from './ledger'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockAutostart, mockNotices, mockTransactions } from './demo'
import { mockConflict } from './commands-vault-files'
import { mockAppSettings, mockCurrencies, mockInfo, mockSettings, mockStatus } from './vault-data'

/** Commands: vault. */
export const vaultCommands: MockCommands = {
  vault_status: (_call) => {
    return resolve(mockStatus.value)
  },
  unlock_vault: ({ body }) => {
    if (String(body?.path).includes('conflict') && !mockConflict.resolved) {
      throw { code: 'SYNC_CONFLICT', message: 'Download/vault.wallet' }
    }
    if (body?.password === 'secret') {
      mockStatus.value = {
        path: String(body?.path ?? mockStatus.value.path),
        exists: true,
        unlocked: true,
        info: mockInfo,
        storage: mockStatus.value.storage,
      }
      return resolve(mockInfo)
    }
    throw { code: 'SECRET', message: 'Wrong password' }
  },
  create_vault: ({ body }) => {
    mockStatus.value = {
      path: String(body?.path ?? mockStatus.value.path),
      exists: true,
      unlocked: true,
      info: mockInfo,
      storage: mockStatus.value.storage,
    }
    return resolve(mockInfo)
  },
  change_password: ({ body }) => {
    if (body?.current_password !== 'secret') throw { code: 'SECRET', message: 'Wrong password' }
    return resolve(undefined)
  },
  // The mock keeps everything in memory: nothing is ever waiting for the disk.
  flush_vault: () => resolve(undefined),
  exit_without_saving: () => resolve(undefined),
  lock_vault_cmd: (_call) => {
    mockStatus.value = { ...mockStatus.value, unlocked: false, info: null }
    return resolve({})
  },
  forget_recent_vault: (_call) => {
    return resolve({})
  },
}

/** Commands: notifications. */
export const notificationsCommands: MockCommands = {
  list_notifications: (_call) => {
    return resolve([...mockNotices].reverse())
  },
  mark_notifications_read: ({ args }) => {
    const keys = args?.keys as string[] | null
    for (const record of mockNotices) if (!keys || keys.includes(record.key)) record.read = true
    return resolve({})
  },
  send_test_notification: (_call) => {
    mockNotices.push({
      key: `test:${Date.now()}`,
      category: 'reminder',
      title: 'Напоминание',
      body: 'Уведомления работают',
      link: null,
      at: new Date().toISOString(),
      delivered_at: new Date().toISOString(),
      read: false,
    })
    return resolve({})
  },
  notification_permission: (_call) => {
    return resolve('granted')
  },
  notification_storage: (_call) => {
    return resolve('memory')
  },
  get_autostart: (_call) => {
    return resolve(mockAutostart.value)
  },
  set_autostart: ({ args }) => {
    mockAutostart.value = Boolean(args?.enabled)
    return resolve(mockAutostart.value)
  },
  update_settings: ({ body }) => {
    const input = body as UpdateSettingsInput
    const settings = mockSettings
    // Like the backend: every field that is present is written, the rest is kept.
    for (const [key, value] of Object.entries(input)) {
      if (value !== null && value !== undefined) Object.assign(settings, { [key]: value })
    }
    // Zero clears a hand-entered month, as in the core.
    for (const key of ['manual_income', 'manual_expenses'] as const) {
      if (input[key] !== null && Number(input[key]) <= 0) settings[key] = null
    }
    if (input.savings_rate || input.daily_budget_mode || input.budget_currency !== null)
      settings.budget_configured = true
    return resolve(settings)
  },
}

/** Same rule as the core's `normalize_codes`: tickers of 2–10 letters or digits, uppercase, in order. */
function normalizeCodes(raw: string[]): string[] {
  const codes = raw.map((code) => code.trim().toUpperCase()).filter((code) => /^(?=.*[A-Z])[A-Z0-9]{2,10}$/.test(code))
  return [...new Set(codes)]
}

/** Commands: app settings (outside the vault). */
export const appSettingsOutsideCommands: MockCommands = {
  // `?platform=android` in the address previews the phone build in the browser.
  app_platform: (_call) => resolve(new URLSearchParams(location.search).get('platform') ?? 'linux'),
  get_app_settings: (_call) => {
    return resolve(mockAppSettings)
  },
  update_app_settings: ({ body }) => {
    const input = body as UpdateAppSettingsInput
    if (input.theme) mockAppSettings.theme = input.theme
    if (input.locale) mockAppSettings.locale = input.locale
    if (input.base_currency) mockAppSettings.base_currency = input.base_currency
    if (input.favorite_currencies) mockAppSettings.favorite_currencies = normalizeCodes(input.favorite_currencies)
    if (input.timezone !== null && input.timezone !== undefined) mockAppSettings.timezone = input.timezone || null
    if (input.inflation) mockAppSettings.inflation = input.inflation
    if (input.enrichment_opt_in !== null && input.enrichment_opt_in !== undefined)
      mockAppSettings.enrichment_opt_in = input.enrichment_opt_in
    if (input.completed_tours) mockAppSettings.completed_tours = [...new Set(input.completed_tours)]
    if (input.seen_whats_new) mockAppSettings.seen_whats_new = [...new Set(input.seen_whats_new)]
    if (input.device_name !== null && input.device_name !== undefined)
      mockAppSettings.device_name = input.device_name.trim() || null
    if (input.sync) mockAppSettings.sync = input.sync
    if (input.minimize_to_tray !== null && input.minimize_to_tray !== undefined)
      mockAppSettings.minimize_to_tray = input.minimize_to_tray
    if (input.fx_refresh_interval_sec) mockAppSettings.fx_refresh_interval_sec = input.fx_refresh_interval_sec
    if (input.display_decimals !== null && input.display_decimals !== undefined)
      mockAppSettings.display_decimals = input.display_decimals
    return resolve(mockAppSettings)
  },
  set_quick_add_shortcut: ({ args }) => {
    const shortcut = String(args?.shortcut ?? '').trim()
    // The browser has no global keys; pretend one combination is taken to show the error.
    if (shortcut === 'Super+Space') throw { code: 'SHORTCUT_FAILED', message: 'already registered' }
    mockAppSettings.quick_add_shortcut = shortcut || null
    return resolve(mockAppSettings)
  },
  get_supported_currencies: (_call) => {
    return resolve({ catalog: mockCurrencies, notice: null })
  },
  list_categories: (_call) => {
    return resolve(mockCategories)
  },
  create_category: ({ body }) => {
    const input = body as CreateCategoryInput
    const key =
      (input.key || input.name)
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '_') || `cat_${Date.now()}`
    const category: Category = { key, name: input.name.trim(), group: input.group, archived: false }
    mockCategories.push(category)
    return resolve(category)
  },
  update_category: ({ body }) => {
    const input = body as UpdateCategoryInput
    const category = mockCategories.find((entry) => entry.key === input.key)
    if (!category) throw { code: 'MISSING', message: 'category' }
    Object.assign(category, { name: input.name?.trim() || null, group: input.group })
    return resolve(category)
  },
  archive_category: ({ args }) => {
    const category = mockCategories.find((entry) => entry.key === String(args?.key))
    if (category) category.archived = Boolean(args?.archived)
    return resolve({})
  },
  delete_category: ({ args }) => {
    const key = String(args?.key)
    if (mockTransactions.some((tx) => txMeta(tx).categories.includes(key)))
      throw { code: 'INVALID_INPUT', message: 'the category is in use; archive it instead' }
    mockCategories.splice(
      mockCategories.findIndex((entry) => entry.key === key),
      1,
    )
    return resolve({})
  },
  list_categorization_rules: (_call) => {
    return resolve([...mockRules].sort((left, right) => left.order - right.order))
  },
  create_categorization_rule: ({ body }) => {
    const input = body as CreateRuleInput
    const rule = {
      id: crypto.randomUUID(),
      pattern: input.pattern,
      category: input.category,
      active: true,
      order: mockRules.length,
    }
    mockRules.push(rule)
    return resolve(rule)
  },
  update_categorization_rule: ({ body }) => {
    const input = body as UpdateRuleInput
    const rule = mockRules.find((entry) => entry.id === input.id)
    if (rule) Object.assign(rule, { pattern: input.pattern, category: input.category, active: input.active })
    return resolve(rule)
  },
  delete_categorization_rule: ({ args }) => {
    mockRules.splice(
      mockRules.findIndex((entry) => entry.id === String(args?.id)),
      1,
    )
    return resolve({})
  },
  reorder_categorization_rules: ({ args }) => {
    const ids = (args?.ids as string[]) ?? []
    for (const rule of mockRules) rule.order = ids.indexOf(rule.id) >= 0 ? ids.indexOf(rule.id) : ids.length
    return resolve({})
  },
}
