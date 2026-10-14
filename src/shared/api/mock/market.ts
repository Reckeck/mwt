/** Prices, FX, losses, flows, net worth, search and candles. */

import { txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { mockCategories, mockContactBalances } from './ledger'
import { mockStores, mockTransactions, priceOf } from './demo'
import { mockAppSettings, mockSettings } from './vault-data'

/** Rough losses in the mock: the fee only, at the mock prices. */
export function mockLossOf(id: string): TxLoss {
  const tx = mockTransactions.find((entry) => txId(entry) === id)
  const fee = tx
    ? txLegs(tx)
        .filter((leg) => leg.role === 'fee')
        .reduce((sum, leg) => sum + Math.abs(Number(leg.amount)) * priceOf(leg.asset.symbol), 0)
    : 0
  return {
    tx_id: id,
    currency: mockAppSettings.base_currency,
    fee: String(fee),
    slippage: '0',
    total: String(fee),
    internal: false,
    complete: true,
  }
}

export function mockFlows(): FlowSummary[] {
  const groups = new Map<string, Transaction[]>()
  for (const tx of mockTransactions) {
    const flow = txMeta(tx).flow_id
    if (flow) groups.set(flow, [...(groups.get(flow) ?? []), tx])
  }
  return [...groups.entries()].map(([flowId, txs]) => {
    const links = [...txs].sort((left, right) => txMeta(left).timestamp.localeCompare(txMeta(right).timestamp))
    const loss = links.reduce((sum, tx) => sum + Number(mockLossOf(txId(tx)).total), 0)
    return {
      flow_id: flowId,
      currency: mockAppSettings.base_currency,
      links: links.map((tx) => ({ tx, loss: mockLossOf(txId(tx)) })),
      started_at: txMeta(links[0]).timestamp,
      ended_at: txMeta(links[links.length - 1]).timestamp,
      gave: [],
      got: [],
      gave_value: '0',
      got_value: '0',
      loss: String(loss),
      difference: String(loss),
    }
  })
}

export function mockFxTable(): FxTable {
  const base = mockAppSettings.base_currency
  const perUsd: Record<string, number> = { usd: 1, rub: 95, eur: 0.92, thb: 36 }
  const baseRate = perUsd[base] ?? 1
  return {
    base,
    rows: Object.entries(perUsd)
      .filter(([code]) => code !== base)
      .map(([code, rate]) => ({
        currency: code,
        per_base: String(rate / baseRate),
        in_base: String(baseRate / rate),
        updated_at: new Date().toISOString(),
        tracked: code === 'rub',
      })),
  }
}

export function analyticsContext() {
  return {
    transactions: mockTransactions,
    categories: mockCategories,
    stores: mockStores,
    priceOf,
    currency: mockSettings.budget_currency || mockAppSettings.base_currency,
  }
}
export function mockCachedPrice(input: AssetPriceInput): CachedPrice {
  const price = priceOf(input.symbol)
  if (!price) return { price_usd: '0', ts: '' }
  return { price_usd: String(price), ts: input.at || new Date().toISOString() }
}

export function mockCalc(input: CalcInput): CalcResult {
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

export const mockSummary: PortfolioSummary = {
  base_currency: 'usd',
  base_fx_as_of: '',
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
      price_base: '9000',
      value_base: '27000',
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
      price_base: '300',
      value_base: '15000',
    },
  ],
  by_chain: [
    { label: 'Bitcoin', value_usd: '27000', value_base: '27000', pct: 64.3 },
    { label: 'Ethereum', value_usd: '15000', value_base: '15000', pct: 35.7 },
  ],
  by_store: [
    {
      // Read when a command runs: the demo wallets do not exist yet while this module loads.
      get store_id() {
        return mockStores[1].id
      },
      name: 'Binance',
      value_usd: '30000',
      value_base: '30000',
      pct: 71,
    },
    {
      // Read when a command runs: the demo wallets do not exist yet while this module loads.
      get store_id() {
        return mockStores[2].id
      },
      name: 'Ledger',
      value_usd: '12000',
      value_base: '12000',
      pct: 29,
    },
  ],
  assets_base: '42050',
  liabilities_base: '0',
  net_worth_base: '42050',
  prices_as_of: new Date(Date.now() - 5 * 60_000).toISOString(),
}

export function mockNetWorth(): NetWorth {
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

export const mockHistory: HistoryPoint[] = [
  { ts: '2026-08-01T00:00:00Z', ts_ms: 1782864000000n, value_usd: '20000', value_base: '20000' },
  { ts: '2026-09-01T00:00:00Z', ts_ms: 1785542400000n, value_usd: '32000', value_base: '32000' },
  { ts: '2026-09-28T00:00:00Z', ts_ms: 1787961600000n, value_usd: '42000', value_base: '42000' },
]

export function mockSearch(rawQuery: string): AssetSearchHit[] {
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

export function mockCandles(input: GetCandlesInput): GetCandlesOutput {
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
