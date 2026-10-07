/// Central typed bridge to the Rust backend.
///
/// Inside Tauri uses `@tauri-apps/api/core`; in a plain browser (hot `pnpm dev`
/// without the desktop shell) falls back to a small in-memory mock so the UI
/// can be iterated on.

import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

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

function mock(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
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
    case 'create_store': {
      const input = body as CreateStoreInput
      const store = {
        id: crypto.randomUUID(),
        name: input.name,
        kind: input.kind,
        color: input.color,
        note: input.note,
        archived: false,
        parent_id: input.parent_id ?? null,
      }
      mockStores.push(store)
      mockTransactions.push(...defaultTxs(input.kind, input.name, store.id))
      return resolve(store)
    }
    case 'update_store': {
      const input = body as UpdateStoreInput
      const store = mockStores.find((candidate) => candidate.id === input.id)
      if (store) Object.assign(store, { name: input.name, color: input.color, note: input.note })
      return resolve(store ?? mockStores[0])
    }
    case 'archive_store': {
      const idx = mockStores.findIndex((candidate) => candidate.id === String(args?.id))
      if (idx >= 0) mockStores[idx].archived = Boolean(args?.archived)
      return resolve({})
    }
    case 'delete_store': {
      const idx = mockStores.findIndex((candidate) => candidate.id === String(args?.id))
      if (idx >= 0) mockStores.splice(idx, 1)
      return resolve({})
    }
    // Tauri v2 converts primitive command args to camelCase, so the mock must
    // read the same names the real backend receives (data *fields* stay snake_case).
    case 'list_transactions':
      return resolve(mockFilterTxs((args?.storeId as string | null) ?? null))
    case 'get_store_overview':
      return resolve(mockOverview(String(args?.storeId ?? '')))
    case 'get_store_history':
      return resolve(mockHistory)
    case 'get_store_pnl':
      return resolve({ total_base: '1234.56', legs: [] })
    case 'get_summary':
      return resolve(mockSummary)
    case 'get_history':
      return resolve(mockHistory)
    case 'get_realized_pnl':
      return resolve({ total_base: '1234.56', legs: [] })
    case 'get_settings':
      return resolve(mockSettings)
    case 'update_settings': {
      const input = body as UpdateSettingsInput
      const settings = mockSettings
      if (input.display_decimals) settings.display_decimals = input.display_decimals
      if (input.fx_refresh_interval_sec) settings.fx_refresh_interval_sec = input.fx_refresh_interval_sec
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
      return resolve(mockAppSettings)
    }
    case 'get_supported_currencies':
      return resolve({ catalog: mockCurrencies, notice: null })
    case 'list_categories':
      return resolve(['salary', 'trading', 'staking', 'other'])
    case 'add_category':
      return resolve(String(body))
    case 'add_transaction':
      return resolve(mockAddTransaction(body as TxnInput))
    case 'add_transfer':
      return resolve(mockAddTransfer(body as TransferInput))
    case 'add_trade':
      return resolve(mockAddTrade(body as TradeInput))
    case 'delete_transaction': {
      const idx = mockTransactions.findIndex((tx) => tx.id === String(args?.id))
      if (idx >= 0) mockTransactions.splice(idx, 1)
      return resolve({})
    }
    case 'update_comment': {
      const input = body as UpdateCommentInput
      const tx = mockTransactions.find((candidate) => candidate.id === input.transaction_id)
      if (tx) tx.comment = input.comment
      return resolve(tx ?? mockTransactions[0])
    }
    case 'search_assets':
      return resolve(mockSearch(body?.query as string))
    case 'get_cached_price':
      return resolve(mockCachedPrice(body as AssetPriceInput))
    case 'calc_preview':
      return resolve(mockCalc(body as CalcInput))
    case 'refresh_all':
      return resolve({ prices: 3, fx: 1, updated_at: new Date().toISOString() })
    case 'set_tx_prices':
      return resolve({})
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
        configured: true,
        hint: apiKey.length <= 8 ? '*'.repeat(apiKey.length) : `${apiKey.slice(0, 4)}…${apiKey.slice(-4)}`,
      }
      return resolve(mockKeyStatus)
    }
    case 'clear_exchange_key':
      mockKeyStatus = { configured: false, hint: null }
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
    case 'list_strategies':
      return resolve([])
    case 'save_strategy':
      return resolve(body?.strategy)
    case 'delete_strategy':
      return resolve({})
    case 'list_runs':
      return resolve([])
    case 'run_strategy': {
      const input = body as RunStrategyInput
      return resolve(mockRun(input.strategy_id))
    }
    case 'get_candles': {
      const input = body as GetCandlesInput
      return resolve(mockCandles(input))
    }
    default:
      return resolve({})
  }
}

let mockKeyStatus: ExchangeKeyStatus = { configured: false, hint: null }

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
}

/** Mirrors what `app_settings.json` would hold. */
const mockAppSettings: AppSettings = {
  theme: 'dark',
  locale: 'auto',
  base_currency: 'usd',
  favorite_currencies: [],
}

const mockCurrencies: SupportedCurrencies = {
  fiat: ['usd', 'eur', 'rub', 'cny', 'gbp', 'jpy', 'chf', 'cad', 'aud'],
  crypto: ['btc', 'eth', 'sol', 'usdt', 'bnb', 'xrp', 'doge'],
}

function defaultTxs(_kind: StoreKind, name: string, storeId: string): Transaction[] {
  const now = Date.now()
  const tx = (offsetHours: number, kind: TxKind, symbol: string, amount: string, comment: string): Transaction => ({
    id: crypto.randomUUID(),
    kind,
    asset: { symbol, name: symbol, coingecko_id: null, chain: 'bitcoin', contract: null, quote: symbol },
    amount,
    store_id: storeId,
    to_store_id: null,
    network: null,
    tx_hash: null,
    category: null,
    comment,
    timestamp: new Date(now - offsetHours * 3.6e6).toISOString(),
    fee: null,
    fee_asset: null,
    group_id: null,
    price_usd: kind === 'income' ? '20000' : null,
    price_base: null,
    fx_override: null,
  })
  return [tx(48, 'income', 'BTC', '4', `Init ${name}`), tx(20, 'income', 'ETH', '135', 'Stacking rewards')]
}

const mockStores: Store[] = [
  {
    id: crypto.randomUUID(),
    name: 'Binance',
    kind: 'exchange',
    color: '#f0b90b',
    archived: false,
    note: 'Фьючи',
    parent_id: null,
  },
  {
    id: crypto.randomUUID(),
    name: 'Ledger',
    kind: 'cold_wallet',
    color: '#3b82f6',
    archived: false,
    note: '',
    parent_id: null,
  },
]

const mockTransactions: Transaction[] = mockStores.flatMap((store) => defaultTxs(store.kind, store.name, store.id))

/** Last known USD prices, mirroring what the Rust `price_cache` would hold. */
const mockPrices: Record<string, number> = { BTC: 90000, ETH: 3000, USD: 1, USDT: 1 }

const priceOf = (symbol: string) => mockPrices[symbol.toUpperCase()] ?? 0

const assetOf = (
  symbol: string,
  name: string,
  coingecko_id: string | null = null,
  chain: Chain | null = 'bitcoin',
  contract: string | null = null,
): AssetRef => ({
  symbol: symbol.toUpperCase(),
  name: name || symbol,
  coingecko_id,
  chain,
  contract,
  quote: symbol.toUpperCase(),
})

function mockFilterTxs(storeId: string | null): Transaction[] {
  const list = storeId
    ? mockTransactions.filter((tx) => tx.store_id === storeId || tx.to_store_id === storeId)
    : mockTransactions
  return [...list].sort((left, right) => right.timestamp.localeCompare(left.timestamp))
}

function mockBaseTx(kind: TxKind, asset: AssetRef, amount: string, storeId: string, comment: string): Transaction {
  return {
    id: crypto.randomUUID(),
    kind,
    asset,
    amount,
    store_id: storeId,
    to_store_id: null,
    network: null,
    tx_hash: null,
    category: null,
    comment,
    timestamp: new Date().toISOString(),
    fee: null,
    fee_asset: null,
    group_id: null,
    price_usd: kind === 'income' ? String(priceOf(asset.symbol)) : null,
    price_base: null,
    fx_override: null,
  }
}

function mockAddTransaction(input: TxnInput): Transaction {
  const asset = assetOf(input.symbol, input.name, input.coingecko_id, input.chain, input.contract)
  const signed = input.kind === 'expense' || input.kind === 'fee' ? `-${input.amount}` : input.amount
  const tx = mockBaseTx(input.kind, asset, signed, input.store_id, input.comment)
  tx.category = input.category ?? null
  tx.network = input.network ?? null
  tx.tx_hash = input.tx_hash ?? null
  tx.fee = input.fee ?? null
  if (input.price_usd) tx.price_usd = input.price_usd
  if (input.timestamp) tx.timestamp = input.timestamp
  mockTransactions.push(tx)
  return tx
}

function mockAddTransfer(input: TransferInput): Transaction[] {
  const asset = assetOf(input.symbol, input.name, input.coingecko_id, input.chain, input.contract)
  const group = crypto.randomUUID()
  const out = mockBaseTx('transfer', asset, `-${input.amount}`, input.from_store_id, input.comment)
  const inn = mockBaseTx('transfer', asset, input.amount, input.to_store_id, input.comment)
  for (const leg of [out, inn]) {
    leg.group_id = group
    leg.category = input.category ?? null
    leg.network = input.network ?? null
    leg.tx_hash = input.tx_hash ?? null
    leg.fee = input.fee ?? null
  }
  out.to_store_id = input.to_store_id
  inn.to_store_id = input.from_store_id
  mockTransactions.push(out, inn)
  return [out, inn]
}

function mockAddTrade(input: TradeInput): Transaction[] {
  const sell = assetOf(
    input.sell_symbol,
    input.sell_name,
    input.sell_coingecko_id,
    input.sell_chain,
    input.sell_contract,
  )
  const buy = assetOf(input.buy_symbol, input.buy_name, input.buy_coingecko_id, input.buy_chain, input.buy_contract)
  const group = crypto.randomUUID()
  const out = mockBaseTx('trade', sell, `-${input.sell_qty}`, input.store_id, input.comment)
  const inn = mockBaseTx('trade', buy, input.buy_qty, input.store_id, input.comment)
  for (const leg of [out, inn]) {
    leg.group_id = group
    leg.category = input.category ?? null
    leg.fee = input.fee ?? null
  }
  mockTransactions.push(out, inn)
  return [out, inn]
}

function mockOverview(storeId: string): StoreOverview {
  const store = mockStores.find((candidate) => candidate.id === storeId) ?? mockStores[0]
  const txs = mockFilterTxs(storeId)
  const byAsset = new Map<string, { asset: AssetRef; qty: number; cost: number }>()

  for (const tx of txs) {
    // a leg belongs to this store when it is booked on it; transfer
    // counterpart legs are netted out
    if (tx.store_id !== storeId) continue
    const sign = tx.kind === 'expense' || tx.kind === 'fee' ? -1 : 1
    const key = tx.asset.symbol.toUpperCase()
    const row = byAsset.get(key) ?? { asset: tx.asset, qty: 0, cost: 0 }
    row.qty += sign * Number(tx.amount)
    if (sign > 0) row.cost = priceOf(key)
    byAsset.set(key, row)
  }

  const positions: StorePosition[] = [...byAsset.values()]
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

  const totalUsd = positions.reduce((acc, position) => acc + Number(position.value_usd), 0)

  return {
    store,
    base_currency: mockAppSettings.base_currency,
    fx_per_usd: '1',
    total_usd: String(totalUsd),
    total_base: String(totalUsd),
    subtree_total_base: String(totalUsd),
    realized_pnl_base: '0',
    positions,
    children: [],
    tx_count: txs.length,
    first_tx_at: txs.length ? txs[txs.length - 1].timestamp : '',
    last_tx_at: txs.length ? txs[0].timestamp : '',
  }
}

function mockCachedPrice(input: AssetPriceInput): CachedPrice {
  const price = priceOf(input.symbol)
  if (!price) return { price_usd: '0', ts: '' }
  return { price_usd: String(price), ts: new Date().toISOString() }
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

function mockRun(strategyId: string): SimRun {
  const point = (step: number, equity: number): EquityPoint => ({
    ts: new Date(Date.now() - (1000 - step) * 3.6e6).toISOString(),
    equity: equity.toFixed(2),
    value: equity.toFixed(2),
  })
  return {
    id: crypto.randomUUID(),
    strategy_id: strategyId,
    started_at: new Date().toISOString(),
    finished_at: new Date().toISOString(),
    initial_capital: '1000',
    final_equity: '1234.56',
    total_return_pct: '23.46',
    max_drawdown_pct: '8.11',
    win_rate_pct: '61.5',
    total_trades: 12,
    equity_curve: Array.from({ length: 80 }, (_, index) => point(index, 1000 + index * 3 + Math.sin(index / 9) * 40)),
    trades: [],
    synthetic: true,
    signals: [],
    candle_params: null,
    granularity_min: 60,
  }
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
    categories: ['salary', 'trading', 'staking', 'other'],
    price_cache: [],
    fx_cache: [],
    pending_imports: [],
    strategies: [],
    runs: [],
  }
}

function resolve<T>(value: T): Promise<T> {
  return new Promise((res) => setTimeout(() => res(structuredClone(value)), 30))
}
