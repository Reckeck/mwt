/** Legs: the `(store, asset)` effects of a transaction, as `Transaction::legs()` in Rust. */

import { body, txKind } from './access'

/** One `(store, asset)` effect, mirroring the Rust `Leg`. */
export interface TxLegView {
  store_id: string
  asset: AssetRef
  /** Signed: positive adds to the store, negative subtracts. */
  amount: string
  role: LegRole
  to_store_id: string | null
  price_usd: string | null
}

export function accountStoreId(ref: AccountRef | null | undefined): string | null {
  return ref && 'store' in ref ? ref.store : null
}

export function accountContactId(ref: AccountRef | null | undefined): string | null {
  return ref && 'contact' in ref ? ref.contact.contact_id : null
}

const negate = (raw: string) => (raw.startsWith('-') ? raw.slice(1) : `-${raw}`)

/** Expand a transaction into its legs, like `Transaction::legs()` in Rust. */
export function txLegs(tx: Transaction): TxLegView[] {
  const legs: TxLegView[] = []
  const push = (
    store: string,
    asset: AssetRef,
    amount: string,
    price: PriceStamp | null,
    role: LegRole = 'principal',
    toStore: string | null = null,
  ) => legs.push({ store_id: store, asset, amount, role, to_store_id: toStore, price_usd: price?.price_usd ?? null })
  const pushFee = (store: string | null, fee: Fee | null) => {
    if (store && fee && Number(fee.amount) !== 0) push(store, fee.asset, negate(fee.amount), fee.price, 'fee')
  }

  switch (txKind(tx)) {
    case 'topup': {
      const record = body(tx, 'topup')
      push(record.store_id, record.asset, record.amount, record.price)
      pushFee(record.store_id, record.fee)
      break
    }
    case 'payment': {
      const record = body(tx, 'payment')
      push(record.store_id, record.asset, negate(record.amount), record.price)
      pushFee(record.store_id, record.fee)
      break
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      const from = accountStoreId(record.from)
      const to = accountStoreId(record.to)
      if (from) push(from, record.asset, negate(record.amount), record.price, 'principal', to)
      if (to) push(to, record.asset, record.amount, record.price, 'principal', from)
      pushFee(from ?? to, record.fee)
      break
    }
    case 'convert': {
      const record = body(tx, 'convert')
      const from = accountStoreId(record.from.account)
      const to = accountStoreId(record.to.account)
      if (from) push(from, record.from.asset, negate(record.from.amount), record.from.price, 'principal', to)
      if (to) push(to, record.to.asset, record.to.amount, record.to.price, 'principal', from)
      pushFee(from ?? to, record.fee)
      break
    }
    case 'trade_spot': {
      const record = body(tx, 'trade_spot')
      push(record.store_id, record.sold.asset, negate(record.sold.qty), record.sold.price)
      push(record.store_id, record.bought.asset, record.bought.qty, record.bought.price)
      pushFee(record.store_id, record.fee)
      break
    }
    case 'trade_futures': {
      const record = body(tx, 'trade_futures')
      const grows = (record.direction === 'long') === (record.action === 'open')
      push(record.store_id, record.contract, grows ? record.quantity : negate(record.quantity), null)
      if (record.action === 'close' && record.closed_pnl && Number(record.closed_pnl) !== 0) {
        push(record.store_id, record.settle, record.closed_pnl, record.price, 'pnl')
      }
      pushFee(record.store_id, record.fee)
      break
    }
    case 'defi': {
      const record = body(tx, 'defi')
      const incoming = ['unstake', 'claim', 'remove_liquidity'].includes(record.action)
      push(record.store_id, record.asset, incoming ? record.amount : negate(record.amount), record.price)
      pushFee(record.store_id, record.fee)
      break
    }
    case 'asset_sell': {
      const record = body(tx, 'asset_sell')
      push(record.store_id, record.asset, negate(record.quantity), null, 'principal', record.proceeds_to)
      if (record.proceeds_to) {
        const proceeds = String(Number(record.unit_price) * Number(record.quantity))
        push(record.proceeds_to, record.currency, proceeds, record.price, 'principal', record.store_id)
      }
      pushFee(record.store_id, record.fee)
      break
    }
    case 'adjustment': {
      const record = body(tx, 'adjustment')
      for (const line of record.lines) {
        const delta = Number(line.after) - Number(line.before)
        if (delta !== 0) push(record.store_id, line.asset, String(delta), line.price, 'adjustment')
      }
      break
    }
  }
  return legs
}

/** Legs that are the movement itself (not fees, not settled PnL); a correction's are its own. */
export function txPrincipalLegs(tx: Transaction): TxLegView[] {
  return txLegs(tx).filter((leg) => leg.role === 'principal' || leg.role === 'adjustment')
}

/** The store the transaction is booked on first. */
export function txStoreId(tx: Transaction): string {
  return txLegs(tx)[0]?.store_id ?? ''
}

/** The principal leg on `storeId`, or the first one. */
export function txLegForStore(tx: Transaction, storeId: string): TxLegView {
  const legs = txPrincipalLegs(tx)
  return legs.find((leg) => leg.store_id === storeId) ?? legs[0] ?? txLegs(tx)[0]
}
