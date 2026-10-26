/** Commands: the vault itself, notifications, app settings outside the vault. */

import { txMeta } from '@/shared/lib/transactions'
import { mockCategories, mockRules } from './ledger'
import { resolve } from './core'
import type { MockCommands } from './core'
import { mockAutostart, mockNotices, mockTransactions } from './demo'
import { mockConflict } from './commands-vault-files'
import { leaveSandbox } from './sandbox-state'
import { mockAppSettings, mockCurrencies, mockInfo, mockPassword, mockSettings, mockStatus } from './vault-data'

/** Commands: vault. */
export const vaultCommands: MockCommands = {
  vault_status: (_call) => {
    return resolve(mockStatus.value)
  },
  unlock_vault: ({ body }) => {
    leaveSandbox()
    if (String(body?.path).includes('conflict') && !mockConflict.resolved) {
      throw { code: 'SYNC_CONFLICT', message: 'Download/vault.wallet' }
    }
    // A vault without a password opens whatever was typed, as the core does.
    if (mockPassword.value === null || body?.password === mockPassword.value) {
      mockStatus.value = {
        path: String(body?.path ?? mockStatus.value.path),
        exists: true,
        unlocked: true,
        info: mockInfo,
        storage: mockStatus.value.storage,
        encrypted: mockInfo.encrypted,
        sandbox: false,
      }
      return resolve(mockInfo)
    }
    throw { code: 'SECRET', message: 'Wrong password' }
  },
  create_vault: ({ body }) => {
    leaveSandbox()
    // `password: null` (or none) makes a vault without a password; an empty one is refused, as in the core.
    if (body?.password === '') throw { code: 'INVALID_INPUT', message: 'the password must not be empty' }
    mockPassword.value = typeof body?.password === 'string' ? body.password : null
    mockInfo.encrypted = mockPassword.value !== null
    if (typeof body?.crypto === 'boolean') mockSettings.crypto = body.crypto
    mockStatus.value = {
      path: String(body?.path ?? mockStatus.value.path),
      exists: true,
      unlocked: true,
      info: mockInfo,
      storage: mockStatus.value.storage,
      encrypted: mockInfo.encrypted,
      sandbox: false,
    }
    return resolve(mockInfo)
  },
  change_password: ({ body }) => {
    if (mockPassword.value === null) {
      throw { code: 'INVALID_INPUT', message: 'the vault has no password to change: set one' }
    }
    if (body?.current_password !== mockPassword.value) throw { code: 'SECRET', message: 'Wrong password' }
    if (!body?.new_password) throw { code: 'INVALID_INPUT', message: 'the new password must not be empty' }
    mockPassword.value = String(body.new_password)
    return resolve(undefined)
  },
  // The mock keeps everything in memory: nothing is ever waiting for the disk.
  flush_vault: () => resolve(undefined),
  exit_without_saving: () => resolve(undefined),
  lock_vault_cmd: (_call) => {
    // Locking the sandbox is leaving it: the demo data goes.
    if (leaveSandbox()) return resolve({})
    mockStatus.value = { ...mockStatus.value, unlocked: false, info: null }
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
    if (input.base_currency) {
      mockAppSettings.base_currency = input.base_currency
      mockAppSettings.base_currency_chosen = true
    }
    if (input.base_currency_chosen !== null && input.base_currency_chosen !== undefined)
      mockAppSettings.base_currency_chosen = input.base_currency_chosen
    if (input.favorite_currencies) mockAppSettings.favorite_currencies = normalizeCodes(input.favorite_currencies)
    if (input.timezone !== null && input.timezone !== undefined) mockAppSettings.timezone = input.timezone || null
    if (input.inflation) mockAppSettings.inflation = input.inflation
    if (input.enrichment_opt_in !== null && input.enrichment_opt_in !== undefined)
      mockAppSettings.enrichment_opt_in = input.enrichment_opt_in
    if (input.completed_tours) mockAppSettings.completed_tours = [...new Set(input.completed_tours)]
    if (input.declined_tours) mockAppSettings.declined_tours = [...new Set(input.declined_tours)]
    if (input.tour_offers !== null && input.tour_offers !== undefined) mockAppSettings.tour_offers = input.tour_offers
    if (input.seen_whats_new) mockAppSettings.seen_whats_new = [...new Set(input.seen_whats_new)]
    if (input.device_name !== null && input.device_name !== undefined)
      mockAppSettings.device_name = input.device_name.trim() || null
    if (input.sync) mockAppSettings.sync = input.sync
    if (input.minimize_to_tray !== null && input.minimize_to_tray !== undefined)
      mockAppSettings.minimize_to_tray = input.minimize_to_tray
    if (input.fx_refresh_interval_sec) mockAppSettings.fx_refresh_interval_sec = input.fx_refresh_interval_sec
    if (input.display_decimals !== null && input.display_decimals !== undefined)
      mockAppSettings.display_decimals = input.display_decimals
    if (input.dashboard_today_collapsed !== null && input.dashboard_today_collapsed !== undefined)
      mockAppSettings.dashboard_today_collapsed = input.dashboard_today_collapsed
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
    const category: Category = {
      key,
      name: input.name.trim(),
      group: input.group,
      archived: false,
      essential: null,
      passive: null,
    }
    mockCategories.push(category)
    return resolve(category)
  },
  update_category: ({ body }) => {
    const input = body as UpdateCategoryInput
    const category = mockCategories.find((entry) => entry.key === input.key)
    if (!category) throw { code: 'MISSING', message: 'category' }
    Object.assign(category, { name: input.name?.trim() || null, group: input.group })
    if (input.essential !== null && input.essential !== undefined) category.essential = input.essential
    if (input.passive !== null && input.passive !== undefined) category.passive = input.passive
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
