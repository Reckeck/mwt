/** What the form holds, and a blank one for a wallet and a kind. */

import { USDT } from '@/entities/asset'

import type { ConvertMode, SplitRow } from '../../transaction-form.types'

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
  /** The edited operation's stored moment (with seconds), kept when `timestamp` is left as it was. */
  originalTimestamp: string
  contactId: string
  price: string
  /** Code `price` is typed in; `''` means the base currency. */
  priceCurrency: string
  flowId: string
  /** The property the operation belongs to (rent, fuel, repair); `''` — none. */
  propertyId: string
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
    originalTimestamp: '',
    contactId: '',
    price: '',
    priceCurrency: '',
    flowId: '',
    propertyId: '',
  }
}
