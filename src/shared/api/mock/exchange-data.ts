/** Mock exchange state: the key, executions, the records a sync brings, syncs. */

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

/**
 * `exec_id`s this mock has already turned into operations of the crypto
 * section. A sync that sees them again reports them as known instead of
 * recording them twice, which is the rule the real conversion holds to
 * (spec E7).
 */
/** Reassigned by commands: a holder, since an imported binding cannot be assigned. */
export const mockRecordedStore: { value: string[] } = { value: [] }
/** Syncs of the mock exchanges, newest first. */
export const mockImportRuns: ImportRun[] = []
export const mockImportSettingsByStore = new Map<string, ImportSettings>()
const mockKeyStatusStoreId = 'mock-store-bybit'

/**
 * What a sweep of the mock exchange reports: two fills, a move between the
 * user's own Bybit accounts and an Earn payout. The shape is still
 * `PendingImport` because that is what the command passes the backend; it is
 * no longer kept anywhere.
 */
export function mockExchangeRecords(storeId = mockKeyStatusStoreId): PendingImport[] {
  const now = Date.UTC(2024, 4, 30, 10, 13, 20)
  const records: PendingImport[] = ['a1', 'a2'].map((suffix, index) => ({
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
  // A move between Bybit accounts and an Earn payout, like a real sweep brings.
  const quiet = { side: '', price: '0', fee: '0', fee_asset: '', is_maker: null, closed_size: null, closed_pnl: null }
  records.push(
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
  return records
}
