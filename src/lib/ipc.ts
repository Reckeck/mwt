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
  const s = typeof raw === 'string' ? raw : String(raw ?? 'Unknown error')
  const i = s.indexOf(': ')
  if (i > 0 && s.slice(0, i) === s.slice(0, i).toUpperCase()) {
    return { code: s.slice(0, i), message: s.slice(i + 2) }
  }
  return { code: 'UNKNOWN', message: s }
}

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) return mock(cmd, args) as Promise<T>
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    throw parseError(e)
  }
}

export async function onEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  // Quietly no-op in browser dev mode.
  if (!isTauri) return () => {}
  return listen<T>(event, (e) => handler(e.payload))
}

// --- browser dev mock --------------------------------------------------------

function mock(cmd: string, args?: Record<string, unknown>): Promise<unknown> {
  const body = args?.input as Record<string, unknown> | undefined
  switch (cmd) {
    case 'vault_status':
      return resolve(mockStatus)
    case 'unlock_vault':
      if (body?.password === 'secret') {
        mockStatus = { ...mockStatus, unlocked: true, info: mockInfo }
        return resolve(mockInfo)
      }
      throw { code: 'SECRET', message: 'Wrong password' }
    case 'create_vault':
      mockStatus = { ...mockStatus, exists: true, unlocked: true, info: mockInfo }
      return resolve(mockInfo)
    case 'lock_vault_cmd':
      mockStatus = { ...mockStatus, unlocked: false, info: null }
      return resolve({})
    case 'list_stores':
      return resolve(mockStores)
    case 'create_store': {
      const input = body as CreateStoreInput
      const store = { id: crypto.randomUUID(), name: input.name, kind: input.kind, color: input.color, note: input.note, archived: false, parent_id: input.parent_id ?? null }
      mockStores.push(store)
      mockTransactions.push(...defaultTxs(input.kind, input.name, store.id))
      return resolve(store)
    }
    case 'update_store': {
      const input = body as UpdateStoreInput
      const s = mockStores.find((x) => x.id === input.id)
      if (s) Object.assign(s, { name: input.name, color: input.color, note: input.note })
      return resolve(s ?? mockStores[0])
    }
    case 'archive_store': {
      const i = mockStores.findIndex((x) => x.id === String(args?.id))
      if (i >= 0) mockStores[i].archived = Boolean(args?.archived)
      return resolve({})
    }
    case 'delete_store': {
      const i = mockStores.findIndex((x) => x.id === String(args?.id))
      if (i >= 0) mockStores.splice(i, 1)
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
      const s = mockSettings
      if (input.base_currency) s.base_currency = input.base_currency
      if (input.theme) s.theme = input.theme
      if (input.locale) s.locale = input.locale
      if (input.display_decimals) s.display_decimals = input.display_decimals
      return resolve(s)
    }
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
      const i = mockTransactions.findIndex((x) => x.id === String(args?.id))
      if (i >= 0) mockTransactions.splice(i, 1)
      return resolve({})
    }
    case 'update_comment': {
      const input = body as UpdateCommentInput
      const tx = mockTransactions.find((x) => x.id === input.transaction_id)
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
      const k = input.api_key
      mockKeyStatus = {
        configured: true,
        hint: k.length <= 8 ? '*'.repeat(k.length) : `${k.slice(0, 4)}…${k.slice(-4)}`,
      }
      return resolve(mockKeyStatus)
    }
    case 'clear_exchange_key':
      mockKeyStatus = { configured: false, hint: null }
      return resolve(mockKeyStatus)
    case 'bybit_executions':
      return resolve(
        mockKeyStatus.configured
          ? { executions: mockExecutions, notice: null }
          : { executions: [], notice: 'no_key' },
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
      const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((i) => i.exec_id)
      const staged = mockPendingImports()
      mockPendingStore = staged.filter((i) => !ids.includes(i.exec_id))
      return resolve({ confirmed: staged.length - mockPendingStore.length })
    }
    case 'discard_pending_imports': {
      const input = body as DiscardPendingImportsInput
      const ids = input.exec_ids.length ? input.exec_ids : mockPendingImports().map((i) => i.exec_id)
      const staged = mockPendingImports()
      mockPendingStore = staged.filter((i) => !ids.includes(i.exec_id))
      return resolve({ discarded: staged.length - mockPendingStore.length })
    }
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
  mockPendingStore = ['a1', 'a2'].map((suffix, i) => ({
    exec_id: `mock-exec-${suffix}`,
    store_id: storeId,
    category: 'spot',
    symbol: 'BTCUSDT',
    side: i === 0 ? 'buy' : 'sell',
    asset_bought: 'BTC',
    qty_bought: i === 0 ? '0.02' : '0',
    asset_sold: 'USDT',
    qty_sold: i === 0 ? '1285.01' : '1300',
    price: '64250.5',
    fee: '0.642505',
    fee_asset: 'USDT',
    occurred_at: new Date(now - i * 86_400_000).toISOString(),
    comment: `bybit spot ${i === 0 ? 'buy' : 'sell'} · mock`,
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
  base_currency: 'usd',
  theme: 'dark',
  locale: 'auto',
  display_decimals: 8,
  favorite_currencies: [],
  dashboard_widgets: [],
  fx_refresh_interval_sec: 300,
  minimize_to_tray: true,
  bybit_import_interval_sec: 900,
}

function defaultTxs(_kind: StoreKind, name: string, storeId: string): Transaction[] {
  const now = Date.now()
  const t = (offsetHours: number, kind: TxKind, symbol: string, amount: string, comment: string): Transaction => ({
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
  return [
    t(48, 'income', 'BTC', '4', `Init ${name}`),
    t(20, 'income', 'ETH', '135', 'Stacking rewards'),
  ]
}

const mockStores: Store[] = [
  { id: crypto.randomUUID(), name: 'Binance', kind: 'exchange', color: '#f0b90b', archived: false, note: 'Фьючи', parent_id: null },
  { id: crypto.randomUUID(), name: 'Ledger', kind: 'cold_wallet', color: '#3b82f6', archived: false, note: '', parent_id: null },
]

const mockTransactions: Transaction[] = mockStores.flatMap((s) => defaultTxs(s.kind, s.name, s.id))

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
    ? mockTransactions.filter((x) => x.store_id === storeId || x.to_store_id === storeId)
    : mockTransactions
  return [...list].sort((a, b) => b.timestamp.localeCompare(a.timestamp))
}

function mockBaseTx(
  kind: TxKind,
  asset: AssetRef,
  amount: string,
  storeId: string,
  comment: string,
): Transaction {
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

function mockAddTransaction(i: TxnInput): Transaction {
  const asset = assetOf(i.symbol, i.name, i.coingecko_id, i.chain, i.contract)
  const signed = i.kind === 'expense' || i.kind === 'fee' ? `-${i.amount}` : i.amount
  const tx = mockBaseTx(i.kind, asset, signed, i.store_id, i.comment)
  tx.category = i.category ?? null
  tx.network = i.network ?? null
  tx.tx_hash = i.tx_hash ?? null
  tx.fee = i.fee ?? null
  if (i.price_usd) tx.price_usd = i.price_usd
  if (i.timestamp) tx.timestamp = i.timestamp
  mockTransactions.push(tx)
  return tx
}

function mockAddTransfer(i: TransferInput): Transaction[] {
  const asset = assetOf(i.symbol, i.name, i.coingecko_id, i.chain, i.contract)
  const group = crypto.randomUUID()
  const out = mockBaseTx('transfer', asset, `-${i.amount}`, i.from_store_id, i.comment)
  const inn = mockBaseTx('transfer', asset, i.amount, i.to_store_id, i.comment)
  for (const leg of [out, inn]) {
    leg.group_id = group
    leg.category = i.category ?? null
    leg.network = i.network ?? null
    leg.tx_hash = i.tx_hash ?? null
    leg.fee = i.fee ?? null
  }
  out.to_store_id = i.to_store_id
  inn.to_store_id = i.from_store_id
  mockTransactions.push(out, inn)
  return [out, inn]
}

function mockAddTrade(i: TradeInput): Transaction[] {
  const sell = assetOf(i.sell_symbol, i.sell_name, i.sell_coingecko_id, i.sell_chain, i.sell_contract)
  const buy = assetOf(i.buy_symbol, i.buy_name, i.buy_coingecko_id, i.buy_chain, i.buy_contract)
  const group = crypto.randomUUID()
  const out = mockBaseTx('trade', sell, `-${i.sell_qty}`, i.store_id, i.comment)
  const inn = mockBaseTx('trade', buy, i.buy_qty, i.store_id, i.comment)
  for (const leg of [out, inn]) {
    leg.group_id = group
    leg.category = i.category ?? null
    leg.fee = i.fee ?? null
  }
  mockTransactions.push(out, inn)
  return [out, inn]
}

function mockOverview(storeId: string): StoreOverview {
  const store = mockStores.find((s) => s.id === storeId) ?? mockStores[0]
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
    .filter((r) => r.qty !== 0)
    .map((r) => {
      const price_usd = priceOf(r.asset.symbol)
      return {
        asset: r.asset,
        quantity: String(r.qty),
        avg_cost: r.cost ? String(r.cost) : null,
        price_usd: String(price_usd),
        value_usd: String(r.qty * price_usd),
        value_base: String(r.qty * price_usd),
        unrealized_base: String(r.qty * (price_usd - r.cost)),
      }
    })
    .sort((a, b) => Number(b.value_base) - Number(a.value_base))

  const total_usd = positions.reduce((acc, p) => acc + Number(p.value_usd), 0)

  return {
    store,
    base_currency: mockSettings.base_currency,
    fx_per_usd: '1',
    total_usd: String(total_usd),
    total_base: String(total_usd),
    subtree_total_base: String(total_usd),
    realized_pnl_base: '0',
    positions,
    children: [],
    tx_count: txs.length,
    first_tx_at: txs.length ? txs[txs.length - 1].timestamp : '',
    last_tx_at: txs.length ? txs[0].timestamp : '',
  }
}

function mockCachedPrice(i: AssetPriceInput): CachedPrice {
  const price = priceOf(i.symbol)
  if (!price) return { price_usd: '0', ts: '' }
  return { price_usd: String(price), ts: new Date().toISOString() }
}

function mockCalc(i: CalcInput): CalcResult {
  const n = (v: string | null | undefined) => Number(v ?? 0)
  const amount = n(i.amount)
  const priceIn = n(i.price_in)
  const priceOut = n(i.price_out)
  const feePct = n(i.fee_pct)
  if (!amount || !priceIn) {
    return { gross_in: '0', fee: '0', spent: '0', gross_out: '0', net_out: '0', exit_value: '0', pnl: '0', roi_pct: '0', risk: '0', risk_pct: '0' }
  }
  const zero = (): CalcResult => ({ gross_in: '0', fee: '0', spent: '0', gross_out: '0', net_out: '0', exit_value: '0', pnl: '0', roi_pct: '0', risk: '0', risk_pct: '0' })
  const gross_in = amount * priceIn
  const fee = (gross_in * feePct) / 100
  const spent = gross_in + fee
  if (i.mode === 'converter') {
    if (!priceOut) return { ...zero(), gross_in: String(gross_in), fee: String(fee), spent: String(spent) }
    const gross_out = amount * priceOut
    return { ...zero(), gross_in: String(gross_in), fee: String(fee), spent: String(spent), gross_out: String(gross_out), net_out: String(gross_out - fee) }
  }
  const out: CalcResult = { ...zero(), gross_in: String(gross_in), fee: String(fee), spent: String(spent) }
  if (i.target_price) {
    const exit = amount * Number(i.target_price)
    const pnl = exit - spent
    out.exit_value = String(exit)
    out.pnl = String(pnl)
    out.roi_pct = String((pnl / spent) * 100)
  }
  if (i.stop_price) {
    out.risk = String(amount * Math.max(priceIn - Number(i.stop_price), 0))
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
    { asset: { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin', chain: 'bitcoin', contract: null, quote: 'BTC' }, quantity: '3', price_usd: '9000', value_usd: '27000' },
    { asset: { symbol: 'ETH', name: 'Ethereum', coingecko_id: 'ethereum', chain: 'ethereum', contract: null, quote: 'ETH' }, quantity: '50', price_usd: '300', value_usd: '15000' },
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

function mockSearch(q: string): AssetSearchHit[] {
  const known = [
    { symbol: 'BTC', name: 'Bitcoin', coingecko_id: 'bitcoin' },
    { symbol: 'ETH', name: 'Ethereum', coingecko_id: 'ethereum' },
    { symbol: 'SOL', name: 'Solana', coingecko_id: 'solana' },
    { symbol: 'USDT', name: 'Tether', coingecko_id: 'tether' },
  ]
  const ql = (q ?? '').toLowerCase()
  return known
    .filter((x) => x.symbol.toLowerCase().includes(ql) || x.name.toLowerCase().includes(ql))
    .map((x) => ({ ...x, chain: 'ethereum', contract: null, rank: 1n }))
}

function mockRun(strategyId: string): SimRun {
  const eq = (i: number, v: number): EquityPoint => ({
    ts: new Date(Date.now() - (1000 - i) * 3.6e6).toISOString(),
    equity: v.toFixed(2),
    value: v.toFixed(2),
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
    equity_curve: Array.from({ length: 80 }, (_, i) => eq(i, 1000 + i * 3 + Math.sin(i / 9) * 40)),
    trades: [],
    synthetic: true,
    signals: [],
    candle_params: null,
    granularity_min: 60,
  }
}

function mockCandles(i: GetCandlesInput): GetCandlesOutput {
  const limit = Math.min(Math.max(i.limit || 300, 1), 1000)
  const stepMs = (i.interval_min || 60) * 60_000
  const now = Math.floor(Date.now() / stepMs) * stepMs
  const basePrice = 60000 + (i.symbol.length * 1500) % 30000
  let price = basePrice
  const candles: CandlePoint[] = []
  for (let k = limit - 1; k >= 0; k--) {
    const drift = Math.sin(k / 17) * 0.004 + Math.cos(k / 6) * 0.002
    const open = price
    const close = open * (1 + drift)
    const high = Math.max(open, close) * (1 + 0.002 * ((k * 7) % 5))
    const low = Math.min(open, close) * (1 - 0.002 * ((k * 3) % 5))
    const volume = String(50 + ((k * 37) % 200))
    candles.push({
      ts: new Date(now - k * stepMs).toISOString(),
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

function resolve<T>(v: T): Promise<T> {
  return new Promise((r) => setTimeout(() => r(structuredClone(v)), 30))
}