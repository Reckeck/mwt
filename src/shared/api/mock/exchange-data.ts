/** Mock exchange state: the key, executions, staged imports, syncs. */

/** Reassigned by commands: a holder, since an imported binding cannot be assigned. */
export const mockKeyStatus: { value: ExchangeKeyStatus } = {
  value: { configured: false, hint: null, spot_store_id: null, futures_store_id: null },
}

/** Scratch-file contents for the "pull from editor" mock round-trip. */
export const mockScratchJson = ''

export const mockExecutions: BybitExecution[] = [
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
/** Reassigned by commands: a holder, since an imported binding cannot be assigned. */
export const mockPendingStore: { value: PendingImport[] } = { value: [] }
/** Syncs of the mock exchanges, newest first. */
export const mockImportRuns: ImportRun[] = []
export const mockImportSettingsByStore = new Map<string, ImportSettings>()
const mockKeyStatusStoreId = 'mock-store-bybit'

export function mockPendingImports(storeId = mockKeyStatusStoreId): PendingImport[] {
  if (mockPendingStore.value.length) return mockPendingStore.value
  const now = Date.now()
  mockPendingStore.value = ['a1', 'a2'].map((suffix, index) => ({
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
    kind: 'fill' as const,
    from_account: null,
    to_account: null,
  }))
  // A move between Bybit accounts and an Earn payout, like a real sweep stages.
  const quiet = { side: '', price: '0', fee: '0', fee_asset: '', is_maker: null, closed_size: null, closed_pnl: null }
  mockPendingStore.value.push(
    {
      ...quiet,
      exec_id: 'transfer:mock-1',
      store_id: storeId,
      category: 'transfer',
      symbol: 'USDT',
      asset_bought: 'USDT',
      qty_bought: '250',
      asset_sold: 'USDT',
      qty_sold: '250',
      occurred_at: new Date(now - 2 * 86_400_000).toISOString(),
      comment: 'Bybit: FUND → UNIFIED',
      kind: 'transfer',
      from_account: 'FUND',
      to_account: 'UNIFIED',
    },
    {
      ...quiet,
      exec_id: 'earn:mock-1',
      store_id: storeId,
      category: 'earn',
      symbol: 'USDT',
      asset_bought: 'USDT',
      qty_bought: '0.42',
      asset_sold: '',
      qty_sold: '0',
      occurred_at: new Date(now - 3 * 86_400_000).toISOString(),
      comment: 'Bybit Earn',
      kind: 'earn_income',
      from_account: null,
      to_account: 'EARN',
    },
  )
  return mockPendingStore.value
}
