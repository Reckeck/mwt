/** `preview_transaction`: the balances a draft would leave, nothing written. */

import { txId, txLegs } from '@/shared/lib/transactions'
import { mockTransactionFrom } from './ledger'
import { mockTransactions, priceOf } from './demo'
import { mockSummary } from './market'
import { mockOwnPositions, mockStore } from './tree'

/** Like `liquidation_price` in Rust: isolated margin, 0.5 % maintenance. */
function mockLiquidation(input: TransactionInput): string | null {
  if (input.type !== 'trade_futures' || input.action !== 'open') return null
  const leverage = Number(input.leverage ?? 0)
  const price = Number(input.fill_price)
  if (!(leverage > 0 && price > 0)) return null
  const share = 1 / leverage - 0.005
  return String(Number((price * (input.direction === 'long' ? 1 - share : 1 + share)).toFixed(8)))
}

/** Like `sale_preview` in Rust: the sold quantity at today's price against the proceeds. */
function mockSale(input: TransactionInput): SalePreview | null {
  if (input.type !== 'asset_sell') return null
  const usd = priceOf(input.asset.symbol)
  const perUsd = priceOf(input.currency.symbol)
  const quantity = Number(input.quantity)
  if (!(usd > 0 && perUsd > 0 && quantity > 0)) return null
  const book = Number(((quantity * usd) / perUsd).toFixed(2))
  const gain = Number((quantity * Number(input.unit_price)).toFixed(2)) - book
  return { currency: input.currency.symbol.toLowerCase(), book_value: String(book), gain: gain.toFixed(2) }
}

/** Like `preview_transaction` in Rust: net legs per store and asset, nothing written. */
export function mockPreview(input: PreviewTransactionInput): TxPreview {
  // Like the backend: a draft that would be refused (no category, no comment)
  // still previews its balances, with the refusal reported in `error`.
  let error: string | null = null
  let draft: Transaction
  try {
    draft = mockTransactionFrom(input.transaction, priceOf)
  } catch (err) {
    const failure = err as { code?: string; message?: string }
    error = `${failure.code ?? 'INVALID_INPUT'}: ${failure.message ?? ''}`
    const common = input.transaction.common
    const patched = {
      ...input.transaction,
      common: {
        ...common,
        comment: common.comment || 'preview',
        categories: common.categories.length ? common.categories : ['other'],
      },
    } as TransactionInput
    try {
      draft = mockTransactionFrom(patched, priceOf)
    } catch {
      return {
        changes: [],
        base_currency: mockSummary.base_currency,
        flow: 'ignored',
        warnings: [],
        error,
        liquidation_price: null,
        sale: null,
      }
    }
  }
  const replaced = input.replaces ? mockTransactions.find((tx) => txId(tx) === input.replaces) : undefined
  const deltas = new Map<string, { storeId: string; asset: AssetRef; delta: number }>()
  const add = (legs: ReturnType<typeof txLegs>, sign: number) => {
    for (const leg of legs) {
      const key = `${leg.store_id}:${leg.asset.symbol.toUpperCase()}`
      const row = deltas.get(key) ?? { storeId: leg.store_id, asset: leg.asset, delta: 0 }
      row.delta += sign * Number(leg.amount)
      deltas.set(key, row)
    }
  }
  add(txLegs(draft), 1)
  if (replaced) add(txLegs(replaced), -1)
  const changes = [...deltas.values()]
    .filter((row) => row.delta !== 0)
    .map((row) => {
      const held = mockOwnPositions(row.storeId).find(
        (position) => position.asset.symbol.toUpperCase() === row.asset.symbol.toUpperCase(),
      )
      const before = Number(held?.quantity ?? 0)
      return {
        store_id: row.storeId,
        store_name: mockStore(row.storeId)?.name ?? '',
        asset: row.asset,
        before: String(before),
        after: String(before + row.delta),
        delta_base: String(row.delta * priceOf(row.asset.symbol)),
      }
    })
  const warnings: PreviewWarning[] = changes
    .filter((change) => Number(change.after) < 0 && mockStore(change.store_id)?.kind !== 'liability')
    .map((change) => ({ kind: 'negative_balance', store_id: change.store_id, asset: change.asset }))
  const flow: FlowClass = 'payment' in draft ? 'daily' : 'topup' in draft ? 'income' : 'ignored'
  return {
    changes,
    base_currency: mockSummary.base_currency,
    flow,
    warnings,
    error,
    liquidation_price: mockLiquidation(input.transaction),
    sale: mockSale(input.transaction),
  }
}
