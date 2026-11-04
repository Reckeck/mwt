/** A transaction built from form input, like the backend would store it. */

import { mockRules } from './seed'

export function asset(ident: AssetIdent): AssetRef {
  const symbol = ident.symbol.trim().toUpperCase()
  return {
    symbol,
    name: ident.name,
    coingecko_id: ident.coingecko_id,
    chain: ident.chain,
    contract: ident.contract,
    quote: symbol,
  }
}

const stamp = (usd: string | null): PriceStamp => ({
  price_usd: usd,
  price_base: null,
  price_base_currency: null,
  fx_override: null,
})
const noStamp = stamp(null)

function fee(input: FeeInput | null): Fee | null {
  return input && Number(input.amount) ? { asset: asset(input.asset), amount: input.amount, price: noStamp } : null
}

export function meta(common: TxCommonInput, id?: string, origin?: TxOrigin): TxMeta {
  return {
    id: id ?? crypto.randomUUID(),
    timestamp: common.timestamp || new Date().toISOString(),
    comment: common.comment.trim(),
    categories: common.categories.map((key) => key.trim().toLowerCase()).filter(Boolean),
    flow_id: common.flow_id,
    contact_id: common.contact_id,
    origin: origin ?? 'manual',
  }
}

/** The backend's rule that matters most for clicking through the form. */
function check(tx: Transaction) {
  const kind = Object.keys(tx)[0] as TxKind
  const record = (tx as unknown as Record<TxKind, { meta: TxMeta }>)[kind]
  if ((kind === 'topup' || kind === 'payment') && !record.meta.categories.length) {
    throw { code: 'INVALID_INPUT', message: 'choose a category' }
  }
}

/** Like `apply_categorization_rules`: fill empty categories from the first matching rule. */
function categorize(tx: Transaction) {
  const kind = Object.keys(tx)[0] as TxKind
  const record = (tx as unknown as Record<TxKind, { meta: TxMeta }>)[kind]
  if (record.meta.categories.length) return
  const text = record.meta.comment.toLowerCase()
  const rule = [...mockRules]
    .sort((left, right) => left.order - right.order)
    .find((candidate) => candidate.active && text.includes(candidate.pattern.toLowerCase()))
  if (rule) record.meta.categories = [rule.category]
}

/**
 * `TransactionInput::to_transaction` plus the service's autocategorize and
 * category check. `priceOf` stands in for the cached quotes.
 */
export function mockTransactionFrom(
  input: TransactionInput,
  priceOf: (symbol: string) => number,
  keep?: { id: string; origin: TxOrigin },
  balanceOf: (storeId: string, symbol: string) => number = () => 0,
): Transaction {
  // The mock has no FX: a typed price is taken as USD whatever its currency.
  const priced = (symbol: string) =>
    input.common.price_usd ?? input.common.price ?? (priceOf(symbol) ? String(priceOf(symbol)) : null)
  const base = meta(input.common, keep?.id, keep?.origin)
  let tx: Transaction
  switch (input.type) {
    case 'topup':
    case 'payment': {
      const main = asset(input.asset)
      const shared = {
        meta: base,
        store_id: input.store_id,
        asset: main,
        amount: input.amount,
        fee: fee(input.fee),
        details: input.details,
        price: stamp(priced(main.symbol)),
      }
      tx =
        input.type === 'topup' ? { topup: { ...shared, from: input.from } } : { payment: { ...shared, to: input.to } }
      break
    }
    case 'transfer': {
      const main = asset(input.asset)
      tx = {
        transfer: {
          meta: base,
          from: input.from,
          to: input.to,
          asset: main,
          amount: input.amount,
          fee: fee(input.fee),
          network: input.network,
          tx_hash: input.tx_hash,
          debt: input.debt,
          price: stamp(priced(main.symbol)),
        },
      }
      break
    }
    case 'convert':
      tx = {
        convert: {
          meta: base,
          from: {
            account: input.from.account,
            asset: asset(input.from.asset),
            amount: input.from.amount,
            price: noStamp,
          },
          to: {
            account: input.to.account,
            asset: asset(input.to.asset),
            amount: input.to.amount,
            price: stamp(priced(input.to.asset.symbol)),
          },
          fee: fee(input.fee),
          rate_override: input.rate_override,
        },
      }
      break
    case 'trade_spot':
      tx = {
        trade_spot: {
          meta: base,
          store_id: input.store_id,
          sold: { asset: asset(input.sold.asset), qty: input.sold.qty, price: stamp(priced(input.sold.asset.symbol)) },
          bought: {
            asset: asset(input.bought.asset),
            qty: input.bought.qty,
            price: stamp(priced(input.bought.asset.symbol)),
          },
          fee: fee(input.fee),
          liquidity: input.liquidity,
          order_id: input.order_id,
        },
      }
      break
    case 'trade_futures': {
      const symbol = input.symbol.trim().toUpperCase()
      tx = {
        trade_futures: {
          meta: base,
          store_id: input.store_id,
          contract: {
            symbol,
            name: symbol,
            coingecko_id: null,
            chain: null,
            contract: `perp:${input.venue}`,
            quote: symbol,
          },
          settle: asset(input.settle),
          direction: input.direction,
          action: input.action,
          quantity: input.quantity,
          fill_price: input.fill_price,
          leverage: input.leverage,
          fee: fee(input.fee),
          closed_pnl: input.closed_pnl,
          order_id: input.order_id,
          category: input.category,
          price: stamp('1'),
        },
      }
      break
    }
    case 'defi': {
      const main = asset(input.asset)
      tx = {
        defi: {
          meta: base,
          store_id: input.store_id,
          action: input.action,
          asset: main,
          amount: input.amount,
          protocol: input.protocol,
          pool_id: input.pool_id,
          network: input.network,
          tx_hash: input.tx_hash,
          fee: fee(input.fee),
          price: stamp(priced(main.symbol)),
        },
      }
      break
    }
    case 'asset_sell':
      tx = {
        asset_sell: {
          meta: base,
          store_id: input.store_id,
          asset: asset(input.asset),
          unit_price: input.unit_price,
          quantity: input.quantity,
          currency: asset(input.currency),
          proceeds_to: input.proceeds_to,
          fee: fee(input.fee),
          price: stamp('1'),
        },
      }
      break
    case 'adjustment': {
      const lines = input.lines
        .map((line) => {
          const before = String(balanceOf(input.store_id, line.asset.symbol))
          return { asset: asset(line.asset), before, after: line.actual, price: noStamp }
        })
        .filter((line) => Number(line.after) !== Number(line.before))
      if (!lines.length) throw { code: 'INVALID_INPUT', message: 'nothing to correct: the balances already match' }
      return { adjustment: { meta: base, store_id: input.store_id, lines, fee: null } }
    }
  }
  categorize(tx)
  check(tx)
  return tx
}
