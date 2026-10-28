/** Prices, FX, losses, flows and net worth; asset search and candles are in `market-assets`. */

import { txId, txLegs, txMeta } from '@/shared/lib/transactions'
import { mockCategories, mockContactBalances } from './ledger'
import { mockStores, mockTransactions, priceOf } from './demo'
import { MOCK_PER_USD } from './fx-rates'
import { mockAppSettings, mockSettings } from './vault-data'
import { ledgerWorth } from './analytics/worth-split'
import { mockScenario } from './scenario'

/** When the FX rows were taken: `stress` has rates from two days ago, 18:05 (Currency-2 · 8); `null` is now. */
let ratesTakenAt: string | null = (() => {
  if (mockScenario !== 'stress') return null
  const day = new Date()
  day.setDate(day.getDate() - 2)
  day.setHours(18, 5, 0, 0)
  return day.toISOString()
})()

/** A refresh brings the rates up to now. */
export function mockRatesRefreshed() {
  ratesTakenAt = null
}

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
  const perUsd = MOCK_PER_USD
  const baseRate = perUsd[base] ?? 1
  const favorites = mockAppSettings.favorite_currencies.map((code) => code.toLowerCase())
  const fiat = Object.entries(perUsd)
    .filter(([code]) => code !== base)
    .map(([code, rate]) => ({
      currency: code,
      per_base: String(rate / baseRate),
      in_base: String(baseRate / rate),
      updated_at: ratesTakenAt ?? new Date().toISOString(),
      tracked: code === 'rub',
      crypto: false,
      favorite: favorites.includes(code),
      change_pct: code === 'rub' ? '0.8' : '-0.2',
    }))
  // Like the core: a crypto favourite with a known price gets a row from that price.
  const coins = favorites
    .filter((code) => !(code in perUsd) && priceOf(code) > 0)
    .map((code) => ({
      currency: code,
      per_base: String(1 / (priceOf(code) * baseRate)),
      in_base: String(priceOf(code) * baseRate),
      updated_at: new Date().toISOString(),
      tracked: false,
      crypto: true,
      favorite: true,
      change_pct: null,
    }))
  return { base, rows: [...fiat, ...coins] }
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

const CALC_ZERO: CalcResult = {
  gross_in: '0',
  fee: '0',
  spent: '0',
  quantity: '0',
  gross_out: '0',
  net_out: '0',
  exit_value: '0',
  pnl: '0',
  roi_pct: '0',
  risk: '0',
  risk_pct: '0',
  reward_risk: '0',
}

/** `calc_preview` in floats: the same formulas as `trade_core::calc::preview`. */
export function mockCalc(input: CalcInput): CalcResult {
  const num = (value: string | null | undefined) => Number(value ?? 0)
  const amount = num(input.amount)
  const priceIn = num(input.price_in)
  const priceOut = num(input.price_out)
  const feePct = num(input.fee_pct)
  if (!amount || !priceIn) return { ...CALC_ZERO }
  if (input.mode === 'converter') {
    const gross_in = amount * priceIn
    const fee = (gross_in * feePct) / 100
    const base = { ...CALC_ZERO, gross_in: String(gross_in), fee: String(fee), spent: String(gross_in + fee) }
    if (!priceOut) return base
    const gross_out = amount * priceOut
    return { ...base, gross_out: String(gross_out), net_out: String(gross_out - fee) }
  }
  const fee = (amount * feePct) / 100
  const quantity = Math.max(amount - fee, 0) / priceIn
  const out: CalcResult = {
    ...CALC_ZERO,
    gross_in: String(amount - fee),
    fee: String(fee),
    spent: String(amount),
    quantity: String(quantity),
  }
  let pnl = 0
  let risk = 0
  if (num(input.target_price) > 0) {
    const exit = quantity * num(input.target_price)
    pnl = exit - amount
    out.exit_value = String(exit)
    out.pnl = String(pnl)
    out.roi_pct = String((pnl / amount) * 100)
  }
  if (num(input.stop_price) > 0) {
    risk = Math.max(amount - quantity * num(input.stop_price), 0)
    out.risk = String(risk)
    out.risk_pct = String((risk / amount) * 100)
  }
  if (risk > 0 && pnl > 0) out.reward_risk = (pnl / risk).toFixed(2)
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
      change_24h_pct: '2.4',
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
      change_24h_pct: '-1.3',
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
  // The ledger's own sum, so the figure, its groups and the report's last point agree.
  const split = ledgerWorth(analyticsContext(), null)
  const receivables = mockContactBalances(mockTransactions).reduce(
    (acc, row) => acc + Number(row.receivable) * priceOf(row.asset.symbol),
    0,
  )
  const liabilities = Math.max(0, split.liabilities)
  return {
    base_currency: 'usd',
    financial: String(split.financial),
    non_financial: String(split.non_financial),
    receivables: String(receivables),
    liabilities: String(liabilities),
    liabilities_by_kind: [],
    payables: '0',
    net: String(split.financial + split.non_financial + receivables - liabilities),
    financial_by_class: [{ class_id: null, amount: String(split.financial) }],
    financial_fund: '0',
  }
}

export const mockHistory: HistoryPoint[] = [
  { ts: '2026-08-01T00:00:00Z', ts_ms: 1782864000000n, value_usd: '20000', value_base: '20000' },
  { ts: '2026-09-01T00:00:00Z', ts_ms: 1785542400000n, value_usd: '32000', value_base: '32000' },
  { ts: '2026-09-28T00:00:00Z', ts_ms: 1787961600000n, value_usd: '42000', value_base: '42000' },
]
