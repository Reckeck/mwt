/// Central typed bridge to the Rust backend.
///
/// Inside Tauri uses `@tauri-apps/api/core`; in a plain browser (hot `pnpm dev`
/// without the desktop shell) falls back to a small in-memory mock so the UI
/// can be iterated on.

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { txContactIds, txId, txLegs, txMeta } from '@/shared/lib/transactions'

import {
  mockCategories,
  mockContactBalances,
  mockContacts,
  mockKindsFor,
  mockRules,
  mockTransactionFrom,
  mockTurnover,
} from './ipc-mock-ledger'
import { mockPlanning } from './ipc-mock-planning'

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export interface AppError {
  code: string
  message: string
}

export function parseError(raw: unknown): AppError {
  const text = typeof raw === 'string' ? raw : String(raw ?? 'Unknown error')
  const sep = text.indexOf(': ')
  if (sep > 0 && text.slice(0, sep) === text.slice(0, sep).toUpperCase()) {
    return { code: text.slice(0, sep), message: text.slice(sep + 2) }
  }
  return { code: 'UNKNOWN', message: text }
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return mock(cmd, args) as Promise<T>
  try {
    return await invoke<T>(cmd, args)
  } catch (err) {
    throw parseError(err)
  }
}

export async function onEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  // Quietly no-op in browser dev mode.
  if (!isTauri) return () => {}
  return listen<T>(event, (received) => handler(received.payload))
}

// --- browser dev mock --------------------------------------------------------

function mock(cmd: string, rawArgs?: Record<string, unknown>): Promise<unknown> {
  seedMock()
  // Like Tauri, which sends arguments as JSON: detaches Vue proxies the mock would otherwise store.
  const args = rawArgs ? (JSON.parse(JSON.stringify(rawArgs)) as Record<string, unknown>) : rawArgs
  const body = args?.input as Record<string, unknown> | undefined
  switch (cmd) {
    case 'vault_status':
      return resolve(mockStatus)
    case 'unlock_vault':
      if (body?.password === 'secret') {
        mockStatus = { path: String(body?.path ?? mockStatus.path), exists: true, unlocked: true, info: mockInfo }
        return resolve(mockInfo)
      }
      throw { code: 'SECRET', message: 'Wrong password' }
    case 'create_vault':
      mockStatus = { path: String(body?.path ?? mockStatus.path), exists: true, unlocked: true, info: mockInfo }
      return resolve(mockInfo)
    case 'lock_vault_cmd':
      mockStatus = { ...mockStatus, unlocked: false, info: null }
      return resolve({})
    case 'pick_vault_file':
      // No native dialog in the browser mock; report a plausible path so the
      // create/open flows stay observable during `pnpm dev`.
      return resolve(args?.save ? '/tmp/mock/new-vault.wallet' : '/tmp/mock/vault.wallet')
    case 'forget_recent_vault':
      return resolve({})
    case 'list_stores':
      return resolve(mockStores)
    case 'get_store_tree':
      return resolve(mockTree())
    case 'store_move_targets':
      return resolve(mockMoveTargets(String(args?.id ?? '')))
    case 'move_store': {
      const input = body as MoveStoreInput
      const store = mockStore(input.id)
      if (!store) throw { code: 'MISSING', message: 'store' }
      const failure = mockPlacementError(input.id, input.parent_id, store.name)
      if (failure) throw failure
      store.parent_id = input.parent_id
      return resolve(store)
    }
    case 'create_store': {
      const input = body as CreateStoreInput
      const failure = mockPlacementError(null, input.parent_id ?? null, input.name)
      if (failure) throw failure
      const store = {
        id: crypto.randomUUID(),
        name: input.name,
        kind: input.kind,
        color: input.color,
        note: input.note,
        archived: false,
        parent_id: input.parent_id ?? null,
        contact_id: input.contact_id ?? null,
        liability: input.liability ?? null,
        // Like the backend: a sub-account starts with its parent's flag, a wallet with its kind's.
        in_budget:
          input.in_budget ??
          (input.parent_id
            ? (mockStore(input.parent_id)?.in_budget ?? false)
            : input.kind === 'cash' || input.kind === 'bank'),
      }
      mockStores.push(store)
      return resolve(store)
    }
    case 'update_store': {
      const input = body as UpdateStoreInput
      const store = mockStore(input.id)
      if (!store) throw { code: 'MISSING', message: 'store' }
      const failure = mockPlacementError(input.id, store.parent_id, input.name)
      if (failure?.code === 'STORE_NAME_TAKEN') throw failure
      Object.assign(store, {
        name: input.name,
        color: input.color,
        note: input.note,
        liability: input.liability,
        contact_id: input.contact_id,
      })
      if (input.in_budget !== null && input.in_budget !== undefined) store.in_budget = input.in_budget
      return resolve(store)
    }
    case 'archive_store': {
      // Like the backend: the whole subtree follows, and a child waits for its parent.
      const id = String(args?.id ?? '')
      const archived = Boolean(args?.archived)
      if (!archived && mockAncestors(id).some((parent) => parent.archived))
        throw { code: 'STORE_RESTORE_PARENT_FIRST', message: 'restore the parent first' }
      for (const target of [id, ...mockSubtreeIds(id)]) {
        const store = mockStore(target)
        if (store) store.archived = archived
      }
      return resolve({})
    }
    case 'delete_store': {
      const idx = mockStores.findIndex((candidate) => candidate.id === String(args?.id))
      if (idx >= 0) mockStores.splice(idx, 1)
      return resolve({})
    }
    // Tauri v2 converts primitive command args to camelCase, so the mock must
    // read the same names the real backend receives (data *fields* stay snake_case).
    case 'list_transactions': {
      const storeId = (args?.storeId as string | null) ?? null
      return resolve(mockFilterTxs(storeId ? mockScope(storeId, args?.includeChildren !== false) : null))
    }
    case 'get_store_overview':
      return resolve(mockOverview(String(args?.storeId ?? ''), args?.includeChildren !== false))
    case 'get_store_history':
      return resolve(mockHistory)
    case 'get_store_pnl':
      return resolve({ total_base: '1234.56', fees_base: '4.5', legs: [] })
    case 'get_summary':
      return resolve(mockSummary)
    case 'get_history':
      return resolve(mockHistory)
    case 'get_realized_pnl':
      return resolve({ total_base: '1234.56', fees_base: '4.5', legs: [] })
    case 'get_settings':
      return resolve(mockSettings)
    case 'update_settings': {
      const input = body as UpdateSettingsInput
      const settings = mockSettings
      // Like the backend: every field that is present is written, the rest is kept.
      for (const [key, value] of Object.entries(input)) {
        if (value !== null && value !== undefined) Object.assign(settings, { [key]: value })
      }
      return resolve(settings)
    }
    // --- app settings (outside the vault) ---
    case 'get_app_settings':
      return resolve(mockAppSettings)
    case 'update_app_settings': {
      const input = body as UpdateAppSettingsInput
      if (input.theme) mockAppSettings.theme = input.theme
      if (input.locale) mockAppSettings.locale = input.locale
      if (input.base_currency) mockAppSettings.base_currency = input.base_currency
      if (input.favorite_currencies) mockAppSettings.favorite_currencies = input.favorite_currencies
      if (input.timezone !== null && input.timezone !== undefined) mockAppSettings.timezone = input.timezone || null
      return resolve(mockAppSettings)
    }
    case 'get_supported_currencies':
      return resolve({ catalog: mockCurrencies, notice: null })
    case 'list_categories':
      return resolve(mockCategories)
    case 'create_category': {
      const input = body as CreateCategoryInput
      const key =
        (input.key || input.name)
          .trim()
          .toLowerCase()
          .replace(/[^a-z0-9]+/g, '_') || `cat_${Date.now()}`
      const category: Category = { key, name: input.name.trim(), group: input.group, archived: false }
      mockCategories.push(category)
      return resolve(category)
    }
    case 'update_category': {
      const input = body as UpdateCategoryInput
      const category = mockCategories.find((entry) => entry.key === input.key)
      if (!category) throw { code: 'MISSING', message: 'category' }
      Object.assign(category, { name: input.name?.trim() || null, group: input.group })
      return resolve(category)
    }
    case 'archive_category': {
      const category = mockCategories.find((entry) => entry.key === String(args?.key))
      if (category) category.archived = Boolean(args?.archived)
      return resolve({})
    }
    case 'delete_category': {
      const key = String(args?.key)
      if (mockTransactions.some((tx) => txMeta(tx).categories.includes(key)))
        throw { code: 'INVALID_INPUT', message: 'the category is in use; archive it instead' }
      mockCategories.splice(
        mockCategories.findIndex((entry) => entry.key === key),
        1,
      )
      return resolve({})
    }
    case 'list_categorization_rules':
      return resolve([...mockRules].sort((left, right) => left.order - right.order))
    case 'create_categorization_rule': {
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
    }
    case 'update_categorization_rule': {
      const input = body as UpdateRuleInput
      const rule = mockRules.find((entry) => entry.id === input.id)
      if (rule) Object.assign(rule, { pattern: input.pattern, category: input.category, active: input.active })
      return resolve(rule)
    }
    case 'delete_categorization_rule':
      mockRules.splice(
        mockRules.findIndex((entry) => entry.id === String(args?.id)),
        1,
      )
      return resolve({})
    case 'reorder_categorization_rules': {
      const ids = (args?.ids as string[]) ?? []
      for (const rule of mockRules) rule.order = ids.indexOf(rule.id) >= 0 ? ids.indexOf(rule.id) : ids.length
      return resolve({})
    }
    // --- contacts ---
    case 'list_contacts':
      return resolve(mockContacts)
    case 'create_contact': {
      const input = args?.input as ContactDraft
      const contact: Contact = {
        id: crypto.randomUUID(),
        name: input.name.trim(),
        kind: input.kind,
        note: input.note,
        accounts: input.accounts.map((account) => ({
          ...account,
          id: account.id || crypto.randomUUID(),
          active: true,
        })),
        archived: false,
        created_at: new Date().toISOString(),
      }
      mockContacts.push(contact)
      return resolve(contact)
    }
    case 'update_contact': {
      const input = body as UpdateContactInput
      const contact = mockContacts.find((entry) => entry.id === input.id)
      if (!contact) throw { code: 'MISSING', message: 'contact' }
      Object.assign(contact, {
        name: input.contact.name.trim(),
        kind: input.contact.kind,
        note: input.contact.note,
        accounts: input.contact.accounts.map((account) => ({
          ...account,
          id: account.id || crypto.randomUUID(),
          active: account.id ? account.active : true,
        })),
      })
      return resolve(contact)
    }
    case 'archive_contact': {
      const contact = mockContacts.find((entry) => entry.id === String(args?.id))
      if (contact) contact.archived = Boolean(args?.archived)
      return resolve({})
    }
    case 'delete_contact': {
      const id = String(args?.id)
      if (mockTransactions.some((tx) => txContactIds(tx).includes(id)))
        throw { code: 'INVALID_INPUT', message: 'the contact is used by transactions; archive it instead' }
      mockContacts.splice(
        mockContacts.findIndex((entry) => entry.id === id),
        1,
      )
      return resolve({})
    }
    case 'get_contact_turnover': {
      const input = body as ContactTurnoverInput
      return resolve(mockTurnover(mockTransactions, input.contact_id, input.account_id, txLegs))
    }
    case 'get_contact_balances':
      return resolve(mockContactBalances(mockTransactions))
    case 'get_net_worth':
      return resolve(mockNetWorth())
    case 'set_manual_price': {
      const input = body as SetManualPriceInput
      return resolve({ ...input, updated_at: new Date().toISOString() })
    }
    case 'clear_manual_price':
      return resolve({})
    // --- transactions ---
    case 'transaction_kinds_for_store_kind':
      return resolve(mockKindsFor(args?.kind as StoreKind))
    case 'add_transaction':
      return resolve(mockPush(mockTransactionFrom(args?.input as TransactionInput, priceOf)))
    case 'update_transaction': {
      const input = body as UpdateTransactionInput
      const index = mockTransactions.findIndex((tx) => txId(tx) === input.id)
      if (index < 0) throw { code: 'MISSING', message: 'transaction' }
      const kept = { id: input.id, origin: txMeta(mockTransactions[index]).origin }
      mockTransactions[index] = mockTransactionFrom(input.transaction, priceOf, kept)
      return resolve(mockTransactions[index])
    }
    case 'delete_transaction': {
      const idx = mockTransactions.findIndex((tx) => txId(tx) === String(args?.id))
      if (idx >= 0) mockTransactions.splice(idx, 1)
      return resolve({})
    }
    case 'search_assets':
      return resolve(mockSearch(body?.query as string))
    case 'get_cached_price':
      return resolve(mockCachedPrice(body as AssetPriceInput))
    case 'convert_amount': {
      const input = body as ConvertAmountInput
      const from = priceOf(input.from)
      const to = priceOf(input.to)
      return resolve({ amount: from && to ? String((Number(input.amount) * from) / to) : null })
    }
    case 'calc_preview':
      return resolve(mockCalc(body as CalcInput))
    case 'refresh_all':
      return resolve({ prices: 3, fx: 1, restamped: 0, updated_at: new Date().toISOString() })
    case 'open_explorer':
      return resolve({})
    // --- exchange (read-only prefill) ---
    case 'inspect_tx':
      // Offline in the browser mock: no suggestions, form left untouched.
      return resolve({ fetched: false, info: null, hints: [], notice: 'unavailable' })
    case 'exchange_key_status':
      return resolve(mockKeyStatus)
    case 'set_exchange_key': {
      const input = body as SetExchangeKeyInput
      const apiKey = input.api_key
      mockKeyStatus = {
        ...mockKeyStatus,
        configured: true,
        hint: apiKey.length <= 8 ? '*'.repeat(apiKey.length) : `${apiKey.slice(0, 4)}…${apiKey.slice(-4)}`,
      }
      return resolve(mockKeyStatus)
    }
    case 'set_exchange_targets': {
      const input = body as SetExchangeTargetsInput
      mockKeyStatus = { ...mockKeyStatus, spot_store_id: input.spot_store_id, futures_store_id: input.futures_store_id }
      return resolve(mockKeyStatus)
    }
    case 'clear_exchange_key':
      mockKeyStatus = { configured: false, hint: null, spot_store_id: null, futures_store_id: null }
      return resolve(mockKeyStatus)
    case 'bybit_executions':
      return resolve(
        mockKeyStatus.configured ? { executions: mockExecutions, notice: null } : { executions: [], notice: 'no_key' },
      )
    // --- bybit import staging (D2.4) ---
    case 'import_bybit_history':
      return resolve(
        mockKeyStatus.configured
          ? { staged: mockPendingImports().length, notice: null }
          : { staged: 0, notice: 'no_key' },
      )
    case 'list_pending_imports':
      return resolve({ imports: mockPendingImports() })
    case 'confirm_pending_imports': {
      const input = body as ConfirmPendingImportsInput
      const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((entry) => entry.exec_id)
      const staged = mockPendingImports()
      mockPendingStore = staged.filter((entry) => !ids.includes(entry.exec_id))
      return resolve({ confirmed: staged.length - mockPendingStore.length })
    }
    case 'discard_pending_imports': {
      const input = body as DiscardPendingImportsInput
      const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((entry) => entry.exec_id)
      const staged = mockPendingImports()
      mockPendingStore = staged.filter((entry) => !ids.includes(entry.exec_id))
      return resolve({ discarded: staged.length - mockPendingStore.length })
    }
    case 'dev_dump_vault':
      return resolve({ json: JSON.stringify(mockVaultDoc(), null, 2) })
    case 'dev_export_vault':
      // No native save dialog in the browser mock; report a plausible path so
      // the success path stays observable during `pnpm dev`.
      return resolve('/tmp/mock/vault.json')
    case 'dev_apply_vault_json': {
      // Reject anything the editor would not accept, exactly like the backend.
      const text = String(args?.json ?? '')
      try {
        JSON.parse(text)
      } catch (err) {
        throw { code: 'DESERIALIZE', message: String(err) }
      }
      return resolve(JSON.stringify(JSON.parse(text), null, 2))
    }
    case 'dev_open_vault_json_in_editor':
      return resolve('/tmp/tradeapp-vault.json')
    case 'dev_reload_vault_json_from_editor':
      // Stands in for the scratch file: whatever the backend last reported.
      return resolve(mockScratchJson)
    case 'get_candles': {
      const input = body as GetCandlesInput
      return resolve(mockCandles(input))
    }
    default: {
      const planned = mockPlanning(
        cmd,
        args,
        {
          transactions: mockTransactions,
          categories: mockCategories,
          stores: mockStores,
          priceOf,
          transactionFrom: (input, ts) =>
            mockTransactionFrom({ ...input, common: { ...input.common, timestamp: ts } } as TransactionInput, priceOf),
          budgetCurrency: mockSettings.budget_currency || mockAppSettings.base_currency,
        },
        mockSettings,
      )
      return resolve(planned ?? {})
    }
  }
}

let mockKeyStatus: ExchangeKeyStatus = { configured: false, hint: null, spot_store_id: null, futures_store_id: null }

/** Scratch-file contents for the "pull from editor" mock round-trip. */
const mockScratchJson = ''

const mockExecutions: BybitExecution[] = [
  {
    symbol: 'BTCUSDT',
    side: 'Buy',
    order_type: 'Market',
    exec_price: '64250.5',
    exec_qty: '0.02',
    exec_value: '1285.01',
    exec_fee: '0.642505',
    exec_time: '1717000000000',
    is_maker: false,
    prefill: {
      side: 'buy',
      symbol: 'BTCUSDT',
      asset_sold: 'USDT',
      qty_sold: '1285.01',
      asset_bought: 'BTC',
      qty_bought: '0.02',
      price: '64250.5',
      fee: '0.642505',
      fee_asset: 'USDT',
      occurred_at: '2024-05-30T10:13:20Z',
      comment: 'bybit spot buy · 0.02 @ 64250.5',
    },
  },
]

/** Staged fills for the browser mock of the D2.4 import flow. */
let mockPendingStore: PendingImport[] = []
const mockKeyStatusStoreId = 'mock-store-bybit'

function mockPendingImports(storeId = mockKeyStatusStoreId): PendingImport[] {
  if (mockPendingStore.length) return mockPendingStore
  const now = Date.now()
  mockPendingStore = ['a1', 'a2'].map((suffix, index) => ({
    exec_id: `mock-exec-${suffix}`,
    store_id: storeId,
    category: 'spot',
    symbol: 'BTCUSDT',
    side: index === 0 ? 'buy' : 'sell',
    asset_bought: 'BTC',
    qty_bought: index === 0 ? '0.02' : '0',
    asset_sold: 'USDT',
    qty_sold: index === 0 ? '1285.01' : '1300',
    price: '64250.5',
    fee: '0.642505',
    fee_asset: 'USDT',
    occurred_at: new Date(now - index * 86_400_000).toISOString(),
    comment: `bybit spot ${index === 0 ? 'buy' : 'sell'} · mock`,
    is_maker: index === 0,
    closed_size: null,
    closed_pnl: null,
  }))
  return mockPendingStore
}

let mockStatus: VaultStatus = {
  path: '/tmp/mock/vault.wallet',
  exists: false,
  unlocked: false,
  info: null,
}

const mockInfo: VaultInfo = {
  path: '/tmp/mock/vault.wallet',
  doc_version: 1,
  id: crypto.randomUUID(),
  created_at: new Date().toISOString(),
  format: 'json',
  base_currency: 'usd',
}

const mockSettings: Settings = {
  display_decimals: 8,
  dashboard_widgets: [],
  fx_refresh_interval_sec: 300,
  minimize_to_tray: true,
  bybit_import_interval_sec: 900,
  savings_rate: '0.10',
  daily_budget_mode: 'month',
  budget_currency: '',
}

/** Mirrors what `app_settings.json` would hold. */
const mockAppSettings: AppSettings = {
  theme: 'dark',
  locale: 'auto',
  base_currency: 'usd',
  favorite_currencies: [],
  timezone: null,
}

const mockCurrencies: SupportedCurrencies = {
  fiat: ['usd', 'eur', 'rub', 'cny', 'gbp', 'jpy', 'chf', 'cad', 'aud'],
  crypto: ['btc', 'eth', 'sol', 'usdt', 'bnb', 'xrp', 'doge'],
}

/** Demo history: a salary, a payment, a trade, a debt and a conversion via an exchanger. */
function demoTransactions(): Transaction[] {
  const now = Date.now()
  const at = (hours: number) => new Date(now - hours * 3.6e6).toISOString()
  const common = (hours: number, comment: string, categories: string[] = []): TxCommonInput => ({
    timestamp: at(hours),
    comment,
    categories,
    flow_id: null,
    contact_id: null,
    price_usd: null,
    fx_override: null,
  })
  const [binance, ledger, futures, bank] = mockStores
  const btc = { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin', chain: 'bitcoin' as Chain, contract: null }
  const usdt = { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether', chain: null, contract: null }
  const rub = { symbol: 'RUB', name: 'Рубль', coingecko_id: null, chain: 'fiat' as Chain, contract: null }
  const details: KindDetails = { cash_breakdown: null, unit_price: null, network: null, tx_hash: null }
  const inputs: TransactionInput[] = [
    {
      type: 'topup',
      common: common(240, 'Зарплата', ['salary']),
      store_id: bank.id,
      asset: rub,
      amount: '150000',
      fee: null,
      from: null,
      details,
    },
    {
      type: 'payment',
      common: common(200, 'Пятёрочка', ['groceries']),
      store_id: bank.id,
      asset: rub,
      amount: '3400',
      fee: null,
      to: { raw: 'Пятёрочка' },
      details,
    },
    {
      type: 'topup',
      common: common(190, 'Депозит', ['transfer']),
      store_id: binance.id,
      asset: usdt,
      amount: '5000',
      fee: null,
      from: null,
      details,
    },
    {
      type: 'trade_spot',
      common: common(150, 'Покупка BTC'),
      store_id: binance.id,
      sold: { asset: usdt, qty: '4500' },
      bought: { asset: btc, qty: '0.05' },
      fee: { asset: usdt, amount: '4.5' },
      liquidity: 'taker',
      order_id: null,
    },
    {
      type: 'transfer',
      common: common(120, 'На холодный'),
      from: { store: binance.id },
      to: { store: ledger.id },
      asset: btc,
      amount: '0.04',
      fee: { asset: btc, amount: '0.0002' },
      network: 'bitcoin',
      tx_hash: null,
      debt: null,
    },
    {
      type: 'transfer',
      common: common(100, 'Маржа на фьючи'),
      from: { store: binance.id },
      to: { store: futures.id },
      asset: usdt,
      amount: '400',
      fee: null,
      network: null,
      tx_hash: null,
      debt: null,
    },
    {
      type: 'transfer',
      common: common(48, 'Маме до зарплаты'),
      from: { store: bank.id },
      to: { contact: { contact_id: 'mock-contact-mom', account_id: 'mock-account-card' } },
      asset: rub,
      amount: '5000',
      fee: null,
      network: null,
      tx_hash: null,
      debt: 'lend',
    },
    {
      type: 'convert',
      common: { ...common(24, 'Обмен через skycapital'), contact_id: 'mock-contact-sky' },
      from: { account: { store: bank.id }, asset: rub, amount: '45000' },
      to: { account: { store: binance.id }, asset: usdt, amount: '480' },
      fee: null,
      rate_override: '93.75',
    },
  ]
  return inputs.map((input) => mockTransactionFrom(input, priceOf))
}

const newStore = (name: string, kind: StoreKind, color: string, parent: string | null = null, note = ''): Store => ({
  id: crypto.randomUUID(),
  name,
  kind,
  color,
  archived: false,
  note,
  parent_id: parent,
  contact_id: null,
  liability: null,
  in_budget: kind === 'cash' || kind === 'bank',
})

const mockStores: Store[] = [
  newStore('Binance', 'exchange', '#f0b90b', null, 'Фьючи'),
  newStore('Ledger', 'cold_wallet', '#3b82f6'),
]
// A sub-account, so the wallet tree has something to show in the browser.
mockStores.push(newStore('Binance Futures', 'exchange', '#f0b90b', mockStores[0].id))
mockStores.push(newStore('Тинькофф', 'bank', '#facc15'))

const mockTransactions: Transaction[] = []

/** Last known USD prices, mirroring what the Rust `price_cache` would hold. */
const mockPrices: Record<string, number> = {
  BTC: 90000,
  ETH: 3000,
  USD: 1,
  USDT: 1,
  RUB: 0.0105,
  EUR: 1.08,
  THB: 0.028,
}

const priceOf = (symbol: string) => mockPrices[symbol.toUpperCase()] ?? 0

const timestampOf = (tx: Transaction) => txMeta(tx).timestamp

function mockFilterTxs(storeIds: string[] | null): Transaction[] {
  const list = storeIds
    ? mockTransactions.filter((tx) => txLegs(tx).some((leg) => storeIds.includes(leg.store_id)))
    : mockTransactions
  return [...list].sort((left, right) => timestampOf(right).localeCompare(timestampOf(left)))
}

function mockPush(tx: Transaction): Transaction {
  mockTransactions.push(tx)
  return tx
}

/** Fill the demo history once every helper above is defined. */
function seedMock() {
  if (!mockTransactions.length) mockTransactions.push(...demoTransactions())
}

/** Mirrors `MAX_STORE_DEPTH` in Rust: a root is depth 0, so three levels. */
const MOCK_MAX_DEPTH = 2

const mockStore = (id: string) => mockStores.find((candidate) => candidate.id === id)
const mockChildren = (id: string) => mockStores.filter((candidate) => candidate.parent_id === id)

/** Parent chain from the root down to `id`'s parent. */
function mockAncestors(id: string): Store[] {
  const chain: Store[] = []
  let parent = mockStore(id)?.parent_id ?? null
  while (parent) {
    const store = mockStore(parent)
    if (!store || chain.includes(store)) break
    chain.unshift(store)
    parent = store.parent_id
  }
  return chain
}

function mockSubtreeIds(id: string): string[] {
  return mockChildren(id).flatMap((child) => [child.id, ...mockSubtreeIds(child.id)])
}

function mockHeight(id: string): number {
  return Math.max(0, ...mockChildren(id).map((child) => mockHeight(child.id) + 1))
}

/** Same rules and codes as `validate_placement` in Rust (R1–R6). */
function mockPlacementError(node: string | null, parent: string | null, name: string): AppError | null {
  if (parent) {
    const target = mockStore(parent)
    if (!target) return { code: 'MISSING', message: 'parent store' }
    if (node && !mockStore(node)?.parent_id) return { code: 'STORE_ROOT_MOVE', message: 'root move' }
    if (node && (parent === node || mockSubtreeIds(node).includes(parent)))
      return { code: 'STORE_CYCLE', message: 'cycle' }
    if (mockAncestors(parent).length + 1 + (node ? mockHeight(node) : 0) > MOCK_MAX_DEPTH)
      return { code: 'STORE_TOO_DEEP', message: 'too deep' }
    if (target.archived) return { code: 'STORE_PARENT_ARCHIVED', message: 'parent archived' }
  }
  const wanted = name.trim().toLowerCase()
  const taken = mockStores.some(
    (store) => store.id !== node && store.parent_id === parent && store.name.trim().toLowerCase() === wanted,
  )
  return taken ? { code: 'STORE_NAME_TAKEN', message: 'name taken' } : null
}

/** Positions held on the store itself, like `store_positions` in Rust. */
function mockOwnPositions(storeId: string): StorePosition[] {
  const byAsset = new Map<string, { asset: AssetRef; qty: number; cost: number }>()
  // Legs carry their own sign, so a trade or transfer needs no special case.
  for (const leg of mockTransactions.flatMap(txLegs)) {
    if (leg.store_id !== storeId || leg.asset.contract?.startsWith('perp:')) continue
    const key = leg.asset.symbol.toUpperCase()
    const row = byAsset.get(key) ?? { asset: leg.asset, qty: 0, cost: 0 }
    row.qty += Number(leg.amount)
    if (Number(leg.amount) > 0) row.cost = priceOf(key)
    byAsset.set(key, row)
  }
  return [...byAsset.values()]
    .filter((row) => row.qty !== 0)
    .map((row) => {
      const priceUsd = priceOf(row.asset.symbol)
      return {
        asset: row.asset,
        quantity: String(row.qty),
        avg_cost: row.cost ? String(row.cost) : null,
        price_usd: String(priceUsd),
        value_usd: String(row.qty * priceUsd),
        value_base: String(row.qty * priceUsd),
        unrealized_base: String(row.qty * (priceUsd - row.cost)),
      }
    })
    .sort((left, right) => Number(right.value_base) - Number(left.value_base))
}

const sumValue = (positions: StorePosition[]) => positions.reduce((acc, row) => acc + Number(row.value_usd), 0)

function mockTreeNode(store: Store, depth: number, parentPath: string[]): StoreTreeNode {
  const path = [...parentPath, store.name]
  const children = mockChildren(store.id)
    .map((child) => mockTreeNode(child, depth + 1, path))
    .sort((left, right) => left.store.name.localeCompare(right.store.name))
  const own = sumValue(mockOwnPositions(store.id))
  const subtree = children.reduce((acc, child) => acc + Number(child.subtree_usd), own)
  return {
    store,
    depth,
    path,
    own_usd: String(own),
    own_base: String(own),
    subtree_usd: String(subtree),
    subtree_base: String(subtree),
    own_tx_count: mockFilterTxs([store.id]).length,
    can_add_child: depth < MOCK_MAX_DEPTH && !store.archived,
    children,
  }
}

function mockTree(): StoreTreeNode[] {
  return mockStores
    .filter((store) => !store.parent_id || !mockStore(store.parent_id))
    .map((store) => mockTreeNode(store, 0, []))
    .sort((left, right) => left.store.name.localeCompare(right.store.name))
}

function mockFindNode(nodes: StoreTreeNode[], id: string): StoreTreeNode | undefined {
  for (const node of nodes) {
    if (node.store.id === id) return node
    const found = mockFindNode(node.children, id)
    if (found) return found
  }
  return undefined
}

const flatNodes = (nodes: StoreTreeNode[]): StoreTreeNode[] =>
  nodes.flatMap((node) => [{ ...node, children: [] }, ...flatNodes(node.children)])

function mockMoveTargets(id: string): StoreMoveTargets {
  const store = mockStore(id)
  if (!store?.parent_id) return { parents: [], can_detach: false }
  return {
    parents: flatNodes(mockTree()).filter(
      (node) => node.store.id !== store.parent_id && !mockPlacementError(id, node.store.id, store.name),
    ),
    can_detach: !mockPlacementError(id, null, store.name),
  }
}

/** Every id a store scope covers. */
const mockScope = (storeId: string, includeChildren: boolean) =>
  includeChildren ? [storeId, ...mockSubtreeIds(storeId)] : [storeId]

function mockOverview(storeId: string, includeChildren: boolean): StoreOverview {
  const store = mockStore(storeId) ?? mockStores[0]
  const node = mockFindNode(mockTree(), store.id) ?? mockTreeNode(store, 0, [])
  const txs = mockFilterTxs(mockScope(store.id, includeChildren))
  // Merged per asset like `merged_positions` in Rust (cost basis left out).
  const merged = new Map<string, StorePosition>()
  for (const row of [store.id, ...mockSubtreeIds(store.id)].flatMap(mockOwnPositions)) {
    const key = row.asset.symbol.toUpperCase()
    const prev = merged.get(key)
    const add = (left: string, right: string) => String(Number(left) + Number(right))
    merged.set(
      key,
      prev
        ? {
            ...prev,
            avg_cost: null,
            quantity: add(prev.quantity, row.quantity),
            value_usd: add(prev.value_usd, row.value_usd),
            value_base: add(prev.value_base, row.value_base),
            unrealized_base: add(prev.unrealized_base, row.unrealized_base),
          }
        : row,
    )
  }
  const subtreePositions = [...merged.values()].sort(
    (left, right) => Number(right.value_base) - Number(left.value_base),
  )
  const subtreeTotal = Number(node.subtree_usd)
  return {
    store,
    ancestors: mockAncestors(store.id).map((parent) => ({ id: parent.id, name: parent.name, color: parent.color })),
    base_currency: mockAppSettings.base_currency,
    fx_per_usd: '1',
    include_children: includeChildren,
    own_usd: node.own_usd,
    own_base: node.own_base,
    subtree_usd: node.subtree_usd,
    subtree_base: node.subtree_base,
    realized_pnl_base: '0',
    positions: mockOwnPositions(store.id),
    subtree_positions: subtreePositions,
    open_positions: [],
    children: node.children.map((child) => ({
      store: child.store,
      value_usd: child.own_usd,
      value_base: child.own_base,
      subtree_base: child.subtree_base,
      share: subtreeTotal ? Number(child.subtree_usd) / subtreeTotal : 0,
    })),
    can_add_child: node.can_add_child,
    tx_count: txs.length,
    first_tx_at: txs.length ? timestampOf(txs[txs.length - 1]) : '',
    last_tx_at: txs.length ? timestampOf(txs[0]) : '',
  }
}

function mockCachedPrice(input: AssetPriceInput): CachedPrice {
  const price = priceOf(input.symbol)
  if (!price) return { price_usd: '0', ts: '' }
  return { price_usd: String(price), ts: input.at || new Date().toISOString() }
}

function mockCalc(input: CalcInput): CalcResult {
  const num = (value: string | null | undefined) => Number(value ?? 0)
  const amount = num(input.amount)
  const priceIn = num(input.price_in)
  const priceOut = num(input.price_out)
  const feePct = num(input.fee_pct)
  if (!amount || !priceIn) {
    return {
      gross_in: '0',
      fee: '0',
      spent: '0',
      gross_out: '0',
      net_out: '0',
      exit_value: '0',
      pnl: '0',
      roi_pct: '0',
      risk: '0',
      risk_pct: '0',
    }
  }
  const zero = (): CalcResult => ({
    gross_in: '0',
    fee: '0',
    spent: '0',
    gross_out: '0',
    net_out: '0',
    exit_value: '0',
    pnl: '0',
    roi_pct: '0',
    risk: '0',
    risk_pct: '0',
  })
  const gross_in = amount * priceIn
  const fee = (gross_in * feePct) / 100
  const spent = gross_in + fee
  if (input.mode === 'converter') {
    if (!priceOut) return { ...zero(), gross_in: String(gross_in), fee: String(fee), spent: String(spent) }
    const gross_out = amount * priceOut
    return {
      ...zero(),
      gross_in: String(gross_in),
      fee: String(fee),
      spent: String(spent),
      gross_out: String(gross_out),
      net_out: String(gross_out - fee),
    }
  }
  const out: CalcResult = { ...zero(), gross_in: String(gross_in), fee: String(fee), spent: String(spent) }
  if (input.target_price) {
    const exit = amount * Number(input.target_price)
    const pnl = exit - spent
    out.exit_value = String(exit)
    out.pnl = String(pnl)
    out.roi_pct = String((pnl / spent) * 100)
  }
  if (input.stop_price) {
    out.risk = String(amount * Math.max(priceIn - Number(input.stop_price), 0))
    out.risk_pct = String((Number(out.risk) / spent) * 100)
  }
  return out
}

const mockSummary: PortfolioSummary = {
  base_currency: 'usd',
  fx_per_usd: '1',
  total_usd: '42000',
  total_base: '42000',
  assets: [
    {
      asset: {
        symbol: 'BTC',
        name: 'Bitcoin',
        coingecko_id: 'bitcoin',
        chain: 'bitcoin',
        contract: null,
        quote: 'BTC',
      },
      quantity: '3',
      price_usd: '9000',
      value_usd: '27000',
    },
    {
      asset: {
        symbol: 'ETH',
        name: 'Ethereum',
        coingecko_id: 'ethereum',
        chain: 'ethereum',
        contract: null,
        quote: 'ETH',
      },
      quantity: '50',
      price_usd: '300',
      value_usd: '15000',
    },
  ],
  by_chain: [
    { label: 'Bitcoin', value_usd: '27000', value_base: '27000', pct: 64.3 },
    { label: 'Ethereum', value_usd: '15000', value_base: '15000', pct: 35.7 },
  ],
  by_store: [
    { store_id: mockStores[0].id, name: 'Binance', value_usd: '30000', value_base: '30000', pct: 71 },
    { store_id: mockStores[1].id, name: 'Ledger', value_usd: '12000', value_base: '12000', pct: 29 },
  ],
  assets_base: '42050',
  liabilities_base: '0',
  net_worth_base: '42050',
}

function mockNetWorth(): NetWorth {
  const financial = Number(mockSummary.total_base)
  const receivables = mockContactBalances(mockTransactions).reduce(
    (acc, row) => acc + Number(row.receivable) * priceOf(row.asset.symbol),
    0,
  )
  return {
    base_currency: 'usd',
    financial: String(financial),
    non_financial: '0',
    receivables: String(receivables),
    liabilities: '0',
    payables: '0',
    net: String(financial + receivables),
  }
}

const mockHistory: HistoryPoint[] = [
  { ts: '2026-08-01T00:00:00Z', ts_ms: 1782864000000n, value_usd: '20000', value_base: '20000' },
  { ts: '2026-09-01T00:00:00Z', ts_ms: 1785542400000n, value_usd: '32000', value_base: '32000' },
  { ts: '2026-09-28T00:00:00Z', ts_ms: 1787961600000n, value_usd: '42000', value_base: '42000' },
]

function mockSearch(rawQuery: string): AssetSearchHit[] {
  const known = [
    { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin' },
    { symbol: 'ETH', name: 'Ethereum', coingecko_id: 'ethereum' },
    { symbol: 'SOL', name: 'Solana', coingecko_id: 'solana' },
    { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether' },
  ]
  const needle = (rawQuery ?? '').toLowerCase()
  return known
    .filter((entry) => entry.symbol.toLowerCase().includes(needle) || entry.name.toLowerCase().includes(needle))
    .map((entry) => ({ ...entry, chain: 'ethereum', contract: null, rank: 1n }))
}

function mockCandles(input: GetCandlesInput): GetCandlesOutput {
  const limit = Math.min(Math.max(input.limit || 300, 1), 1000)
  const stepMs = (input.interval_min || 60) * 60_000
  const now = Math.floor(Date.now() / stepMs) * stepMs
  const basePrice = 60000 + ((input.symbol.length * 1500) % 30000)
  let price = basePrice
  const candles: CandlePoint[] = []
  for (let step = limit - 1; step >= 0; step--) {
    const drift = Math.sin(step / 17) * 0.004 + Math.cos(step / 6) * 0.002
    const open = price
    const close = open * (1 + drift)
    const high = Math.max(open, close) * (1 + 0.002 * ((step * 7) % 5))
    const low = Math.min(open, close) * (1 - 0.002 * ((step * 3) % 5))
    const volume = String(50 + ((step * 37) % 200))
    candles.push({
      ts: new Date(now - step * stepMs).toISOString(),
      open: open.toFixed(2),
      high: high.toFixed(2),
      low: low.toFixed(2),
      close: close.toFixed(2),
      volume,
    })
    price = close
  }
  return { candles }
}

/** Stand-in for the decrypted `VaultDoc`, mirroring the Rust dump command. */
function mockVaultDoc(): VaultDoc {
  return {
    version: 1,
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
  }
}

function resolve<T>(value: T): Promise<T> {
  return new Promise((res) => setTimeout(() => res(structuredClone(value)), 30))
}
