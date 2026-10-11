import { body, txKind } from '@/shared/lib/transactions'
import { parseDecimal } from '@/shared/lib/decimal'

import type { ConvertMode, SplitRow } from './transaction-form.types'
import { toIsoTimestamp, toLocalInput } from '@/shared/lib/datetime'

/**
 * Everything the transaction form edits, flat and string-typed the way the
 * inputs hold it. `formToInput` turns it into the backend's `TransactionInput`
 * and `formFromTransaction` loads a stored transaction back for editing; the
 * rules themselves (what is required, what fits which wallet) live in Rust.
 */
export interface TransactionFormState {
  kind: TxKind
  /** The wallet the form was opened on. */
  storeId: string

  // topup · payment · transfer · defi
  asset: AssetRef | null
  amount: string
  /** Topup: who paid. Payment: who was paid. */
  counterpart: AccountRef | null
  /** Transfer ends. */
  from: AccountRef | null
  to: AccountRef | null
  debt: DebtEffect | null
  network: Chain | ''
  txHash: string
  /** Cashback: the card whose purchases earned it. */
  sourceStoreId: string
  /** Property bought (topup on an `asset` store): price of one unit, base currency. */
  unitPrice: string

  // convert
  convertMode: ConvertMode
  /** P2P: who the money goes through (a contact, their card, or text). */
  via: AccountRef | null
  /** More recipients of an exchange or a top-up, saved with it as one chain. */
  extra: SplitRow[]
  fromAsset: AssetRef | null
  fromAmount: string
  toAsset: AssetRef | null
  toAmount: string
  rate: string

  // trade_spot
  soldAsset: AssetRef | null
  soldQty: string
  boughtAsset: AssetRef | null
  boughtQty: string
  liquidity: Liquidity | ''
  orderId: string

  // trade_futures
  venue: string
  symbol: string
  settle: AssetRef | null
  direction: FuturesDirection
  action: FuturesAction
  quantity: string
  fillPrice: string
  leverage: string
  closedPnl: string
  futuresCategory: FuturesCategory

  // defi
  defiAction: DefiAction
  protocol: string
  poolId: string

  // asset_sell
  sellPrice: string
  sellQty: string
  currency: AssetRef | null
  proceedsTo: string

  // shared
  feeAmount: string
  /** Asset of the fee; empty = the main asset of the kind. */
  feeAsset: AssetRef | null
  categories: string[]
  comment: string
  /** `datetime-local` value; empty = now. */
  timestamp: string
  contactId: string
  price: string
  /** Code `price` is typed in; `''` means the base currency. */
  priceCurrency: string
  flowId: string
}

export const USDT: AssetRef = {
  symbol: 'USDT',
  name: 'Tether',
  coingecko_id: 'tether',
  chain: null,
  contract: null,
  quote: 'USDT',
}

export function emptyForm(storeId: string, kind: TxKind): TransactionFormState {
  return {
    kind,
    storeId,
    asset: null,
    amount: '',
    counterpart: null,
    from: { store: storeId },
    to: null,
    debt: null,
    network: '',
    txHash: '',
    sourceStoreId: '',
    unitPrice: '',
    fromAsset: null,
    fromAmount: '',
    toAsset: null,
    toAmount: '',
    rate: '',
    convertMode: 'direct',
    via: null,
    extra: [],
    soldAsset: null,
    soldQty: '',
    boughtAsset: null,
    boughtQty: '',
    liquidity: '',
    orderId: '',
    venue: 'bybit',
    symbol: '',
    settle: USDT,
    direction: 'long',
    action: 'open',
    quantity: '',
    fillPrice: '',
    leverage: '',
    closedPnl: '',
    futuresCategory: 'linear',
    defiAction: 'stake',
    protocol: '',
    poolId: '',
    sellPrice: '',
    sellQty: '',
    currency: null,
    proceedsTo: '',
    feeAmount: '',
    feeAsset: null,
    categories: [],
    comment: '',
    timestamp: '',
    contactId: '',
    price: '',
    priceCurrency: '',
    flowId: '',
  }
}

const text = (raw: string): string | null => raw.trim() || null

function ident(asset: AssetRef): AssetIdent {
  return {
    symbol: asset.symbol,
    name: asset.name,
    coingecko_id: asset.coingecko_id,
    chain: asset.chain,
    contract: asset.contract,
  }
}

/** The asset a fee defaults to: what the kind mainly moves. */
function mainAsset(form: TransactionFormState): AssetRef | null {
  switch (form.kind) {
    case 'convert':
      return form.fromAsset
    case 'trade_spot':
      return form.boughtAsset
    case 'trade_futures':
      return form.settle
    case 'asset_sell':
      return form.currency
    default:
      return form.asset
  }
}

function feeInput(form: TransactionFormState): FeeInput | null {
  const asset = form.feeAsset ?? mainAsset(form)
  if (!form.feeAmount.trim() || Number(form.feeAmount) === 0 || !asset) return null
  return { asset: ident(asset), amount: form.feeAmount.trim() }
}

/** The asset `common.price_usd` prices: the one received, for two-asset kinds. */
export function pricedAsset(form: TransactionFormState): AssetRef | null {
  switch (form.kind) {
    case 'convert':
      return form.toAsset
    case 'trade_spot':
      return form.boughtAsset
    case 'trade_futures':
    case 'asset_sell':
      return null
    default:
      return form.asset
  }
}

/** A stored stamp back into the form: the base-currency price when it was typed so, else USD. */
function stampToPrice(stamp: PriceStamp): Pick<TransactionFormState, 'price' | 'priceCurrency'> {
  if (stamp.price_base) return { price: stamp.price_base, priceCurrency: '' }
  return { price: stamp.price_usd ?? '', priceCurrency: stamp.price_usd ? 'usd' : '' }
}

/** P2P through a contact: the operation is with them. */
function viaContact(via: AccountRef | null): string | null {
  return via && 'contact' in via ? via.contact.contact_id : null
}

/** P2P through someone named in text: said in the comment. */
function viaComment(via: AccountRef | null, comment: string): string {
  return via && 'raw' in via ? [`P2P: ${via.raw}`, comment].filter(Boolean).join(' · ') : comment
}

function common(form: TransactionFormState): TxCommonInput {
  return {
    timestamp: toIsoTimestamp(form.timestamp),
    comment: form.comment.trim(),
    categories: [...form.categories],
    flow_id: text(form.flowId),
    contact_id: text(form.contactId),
    // The backend turns `price` into USD at the rate of the operation's moment.
    price_usd: null,
    fx_override: null,
    price: text(form.price),
    price_currency: text(form.priceCurrency),
  }
}

function details(form: TransactionFormState): KindDetails {
  return {
    cash_breakdown: null,
    unit_price: text(form.unitPrice),
    network: form.network || null,
    tx_hash: text(form.txHash),
    source_store_id: (form.categories.includes('cashback') && text(form.sourceStoreId)) || undefined,
  }
}

/** Build the backend input, or `null` while a required asset is missing. */
export function formToInput(form: TransactionFormState): TransactionInput | null {
  const shared = { common: common(form), fee: feeInput(form) }
  switch (form.kind) {
    case 'topup':
      if (!form.asset) return null
      return {
        type: 'topup',
        ...shared,
        store_id: form.storeId,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        from: form.counterpart,
        details: details(form),
      }
    case 'payment':
      if (!form.asset) return null
      return {
        type: 'payment',
        ...shared,
        store_id: form.storeId,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        to: form.counterpart,
        details: details(form),
      }
    case 'transfer':
      if (!form.asset || !form.from || !form.to) return null
      return {
        type: 'transfer',
        ...shared,
        from: form.from,
        to: form.to,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        network: form.network || null,
        tx_hash: text(form.txHash),
        debt: form.debt,
      }
    case 'convert': {
      if (!form.fromAsset || !form.toAsset || !form.from || !form.to) return null
      const p2p = form.convertMode === 'p2p'
      return {
        type: 'convert',
        ...shared,
        common: p2p
          ? {
              ...shared.common,
              contact_id: viaContact(form.via) ?? shared.common.contact_id,
              comment: viaComment(form.via, shared.common.comment),
            }
          : shared.common,
        from: { account: form.from, asset: ident(form.fromAsset), amount: form.fromAmount.trim() },
        to: { account: form.to, asset: ident(form.toAsset), amount: form.toAmount.trim() },
        rate_override: text(form.rate),
      }
    }
    case 'trade_spot':
      if (!form.soldAsset || !form.boughtAsset) return null
      return {
        type: 'trade_spot',
        ...shared,
        store_id: form.storeId,
        sold: { asset: ident(form.soldAsset), qty: form.soldQty.trim() },
        bought: { asset: ident(form.boughtAsset), qty: form.boughtQty.trim() },
        liquidity: form.liquidity || null,
        order_id: text(form.orderId),
      }
    case 'trade_futures':
      if (!form.settle) return null
      return {
        type: 'trade_futures',
        ...shared,
        store_id: form.storeId,
        venue: form.venue.trim() || 'bybit',
        symbol: form.symbol.trim().toUpperCase(),
        settle: ident(form.settle),
        direction: form.direction,
        action: form.action,
        quantity: form.quantity.trim(),
        fill_price: form.fillPrice.trim(),
        leverage: text(form.leverage),
        closed_pnl: form.action === 'close' ? text(form.closedPnl) : null,
        order_id: text(form.orderId),
        category: form.futuresCategory,
      }
    case 'defi':
      if (!form.asset) return null
      return {
        type: 'defi',
        ...shared,
        store_id: form.storeId,
        action: form.defiAction,
        asset: ident(form.asset),
        amount: form.amount.trim(),
        protocol: text(form.protocol),
        pool_id: text(form.poolId),
        network: form.network || null,
        tx_hash: text(form.txHash),
      }
    case 'asset_sell':
      if (!form.asset || !form.currency) return null
      return {
        type: 'asset_sell',
        ...shared,
        store_id: form.storeId,
        asset: ident(form.asset),
        unit_price: form.sellPrice.trim(),
        quantity: form.sellQty.trim(),
        currency: ident(form.currency),
        proceeds_to: text(form.proceedsTo),
      }
    // A correction has its own screen (`AdjustmentDialog`).
    case 'adjustment':
      return null
  }
}

/** Kinds that may be split between several recipients. */
export const SPLITTABLE: TxKind[] = ['convert', 'topup']

/** Whether the form saves as several linked operations. */
export function isLinked(form: TransactionFormState): boolean {
  return SPLITTABLE.includes(form.kind) && form.extra.length > 0
}

/**
 * The main operation and one per extra recipient: an exchange gives each its
 * share of the same asset from the same account; a top-up puts the same
 * asset on another account. `null` while something required is missing.
 */
export function formToInputs(form: TransactionFormState): TransactionInput[] | null {
  const main = formToInput(form)
  if (!main) return null
  if (!isLinked(form)) return [main]
  if (main.type === 'convert') return splitConvert(form, main)
  const out: TransactionInput[] = [main]
  for (const row of form.extra) {
    if (main.type !== 'topup' || !row.to || !('store' in row.to)) return null
    out.push({ ...main, store_id: row.to.store, amount: row.receive.trim() })
  }
  return out
}

/** A decimal string with at most `places` digits after the point, no trailing zeros. */
function trimmed(value: number, places: number): string {
  return String(Number(value.toFixed(places)))
}

/**
 * One exchange paid out to several accounts: one operation per account, the
 * given amount split in proportion to what lands on each; the first takes
 * the rounding rest so the parts add up to exactly what was given.
 */
function splitConvert(form: TransactionFormState, main: Extract<TransactionInput, { type: 'convert' }>) {
  const rows = [{ to: form.to, receive: form.toAmount }, ...form.extra]
  const received = rows.map((row) => Number(parseDecimal(row.receive) ?? Number.NaN))
  const total = received.reduce((sum, value) => sum + value, 0)
  const given = Number(parseDecimal(form.fromAmount) ?? Number.NaN)
  if (!(total > 0 && given > 0) || received.some((value) => !(value > 0))) return null
  const places = Math.max(2, form.fromAmount.split(/[.,]/)[1]?.length ?? 0)
  const shares = received.map((value) => Number((given * (value / total)).toFixed(places)))
  shares[0] = Number((given - shares.slice(1).reduce((sum, value) => sum + value, 0)).toFixed(places))
  const out: TransactionInput[] = []
  for (const [index, row] of rows.entries()) {
    if (!row.to) return null
    out.push({
      ...main,
      from: { ...main.from, amount: trimmed(shares[index], places) },
      to: { ...main.to, account: row.to, amount: row.receive.trim() },
      rate_override: index === 0 ? main.rate_override : null,
    })
  }
  return out
}

/**
 * What still has to be filled before the form can be saved, as i18n keys
 * (`tx.missing.*`) — said under the form instead of a silently grey button.
 */
export function missingFields(form: TransactionFormState): string[] {
  const out: string[] = []
  const blank = (value: string) => parseDecimal(value) === null || Number(value.replace(',', '.')) <= 0
  switch (form.kind) {
    case 'convert':
      if (!form.from) out.push('from')
      if (!form.fromAsset) out.push('fromAsset')
      if (blank(form.fromAmount)) out.push('fromAmount')
      if (form.convertMode === 'p2p' && !form.via) out.push('via')
      if (!form.to) out.push('to')
      if (!form.toAsset) out.push('toAsset')
      if (blank(form.toAmount)) out.push('toAmount')
      break
    case 'transfer':
      if (!form.asset) out.push('asset')
      if (blank(form.amount)) out.push('amount')
      if (!form.from) out.push('from')
      if (!form.to) out.push('to')
      break
    case 'trade_spot':
      if (!form.soldAsset) out.push('soldAsset')
      if (blank(form.soldQty)) out.push('soldQty')
      if (!form.boughtAsset) out.push('boughtAsset')
      if (blank(form.boughtQty)) out.push('boughtQty')
      break
    case 'trade_futures':
      if (!form.symbol.trim()) out.push('symbol')
      if (blank(form.quantity)) out.push('quantity')
      if (blank(form.fillPrice)) out.push('fillPrice')
      break
    case 'asset_sell':
      if (!form.asset) out.push('asset')
      if (blank(form.sellQty)) out.push('sellQty')
      if (blank(form.sellPrice)) out.push('sellPrice')
      if (!form.currency) out.push('currency')
      break
    case 'adjustment':
      break
    default:
      if (!form.asset) out.push('asset')
      if (blank(form.amount)) out.push('amount')
  }
  if (SPLITTABLE.includes(form.kind) && form.extra.some((row) => !row.to || blank(row.receive))) out.push('extra')
  return out
}

/** Whether every field the kind cannot do without is filled (the backend checks the rest). */
export function isFormComplete(form: TransactionFormState): boolean {
  if (!formToInputs(form) || missingFields(form).length) return false
  const filled = (...values: string[]) => values.every((value) => value.trim() !== '')
  switch (form.kind) {
    case 'convert':
      return filled(form.fromAmount, form.toAmount)
    case 'trade_spot':
      return filled(form.soldQty, form.boughtQty)
    case 'trade_futures':
      return filled(form.symbol, form.quantity, form.fillPrice)
    case 'asset_sell':
      return filled(form.sellPrice, form.sellQty)
    default:
      return filled(form.amount)
  }
}

/** Load a stored transaction into the form for editing. */
export function formFromTransaction(tx: Transaction, storeId: string): TransactionFormState {
  const kind = txKind(tx)
  const form = emptyForm(storeId, kind)
  const meta = (tx as unknown as Record<TxKind, { meta: TxMeta; fee: Fee | null }>)[kind]
  form.categories = [...meta.meta.categories]
  form.comment = meta.meta.comment
  form.timestamp = toLocalInput(meta.meta.timestamp)
  form.contactId = meta.meta.contact_id ?? ''
  form.flowId = meta.meta.flow_id ?? ''
  if (meta.fee) {
    form.feeAmount = meta.fee.amount
    form.feeAsset = meta.fee.asset
  }
  switch (kind) {
    case 'topup': {
      const record = body(tx, 'topup')
      Object.assign(form, {
        storeId: record.store_id,
        asset: record.asset,
        amount: record.amount,
        counterpart: record.from,
        network: record.details.network ?? '',
        txHash: record.details.tx_hash ?? '',
        sourceStoreId: record.details.source_store_id ?? '',
        unitPrice: record.details.unit_price ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'payment': {
      const record = body(tx, 'payment')
      Object.assign(form, {
        storeId: record.store_id,
        asset: record.asset,
        amount: record.amount,
        counterpart: record.to,
        network: record.details.network ?? '',
        txHash: record.details.tx_hash ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      Object.assign(form, {
        asset: record.asset,
        amount: record.amount,
        from: record.from,
        to: record.to,
        debt: record.debt,
        network: record.network ?? '',
        txHash: record.tx_hash ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'convert': {
      const record = body(tx, 'convert')
      Object.assign(form, {
        from: record.from.account,
        to: record.to.account,
        fromAsset: record.from.asset,
        fromAmount: record.from.amount,
        toAsset: record.to.asset,
        toAmount: record.to.amount,
        rate: record.rate_override ?? '',
        ...stampToPrice(record.to.price),
      })
      break
    }
    case 'trade_spot': {
      const record = body(tx, 'trade_spot')
      Object.assign(form, {
        storeId: record.store_id,
        soldAsset: record.sold.asset,
        soldQty: record.sold.qty,
        boughtAsset: record.bought.asset,
        boughtQty: record.bought.qty,
        liquidity: record.liquidity ?? '',
        orderId: record.order_id ?? '',
        ...stampToPrice(record.bought.price),
      })
      break
    }
    case 'trade_futures': {
      const record = body(tx, 'trade_futures')
      const venue = record.contract.contract?.replace(/^perp:/, '') ?? 'bybit'
      Object.assign(form, {
        storeId: record.store_id,
        venue,
        symbol: record.contract.symbol,
        settle: record.settle,
        direction: record.direction,
        action: record.action,
        quantity: record.quantity,
        fillPrice: record.fill_price,
        leverage: record.leverage ?? '',
        closedPnl: record.closed_pnl ?? '',
        orderId: record.order_id ?? '',
        futuresCategory: record.category,
      })
      break
    }
    case 'defi': {
      const record = body(tx, 'defi')
      Object.assign(form, {
        storeId: record.store_id,
        defiAction: record.action,
        asset: record.asset,
        amount: record.amount,
        protocol: record.protocol ?? '',
        poolId: record.pool_id ?? '',
        network: record.network ?? '',
        txHash: record.tx_hash ?? '',
        ...stampToPrice(record.price),
      })
      break
    }
    case 'asset_sell': {
      const record = body(tx, 'asset_sell')
      Object.assign(form, {
        storeId: record.store_id,
        asset: record.asset,
        sellPrice: record.unit_price,
        sellQty: record.quantity,
        currency: record.currency,
        proceedsTo: record.proceeds_to ?? '',
      })
      break
    }
  }
  return form
}

/** Which groups a kind's categories come from (mirrors V12 in Rust). */
export function categoryGroupsFor(kind: TxKind): CategoryGroup[] {
  if (kind === 'topup') return ['income', 'service']
  if (kind === 'payment') return ['fixed', 'daily', 'savings', 'service']
  return ['income', 'fixed', 'daily', 'savings', 'service']
}

/**
 * The debt effect a transfer can carry, from its direction: money leaving my
 * wallet lends or repays, money arriving borrows or collects.
 */
export function debtChoices(form: TransactionFormState): DebtEffect[] {
  const fromContact = form.from !== null && 'contact' in form.from
  const toContact = form.to !== null && 'contact' in form.to
  if (toContact && form.from && 'store' in form.from) return ['lend', 'repay']
  if (fromContact && form.to && 'store' in form.to) return ['borrow', 'collect']
  return []
}

export { toIsoTimestamp, toLocalInput } from '@/shared/lib/datetime'
