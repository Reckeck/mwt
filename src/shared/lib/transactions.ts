/**
 * Reading a `Transaction` without matching on it everywhere.
 *
 * The backend `Transaction` is a closed union — one key per kind, each variant
 * carrying only the fields that kind has:
 *
 * ```ts
 * type Transaction = { topup: TopupTx } | { transfer: TransferTx } | ...
 * ```
 *
 * This module is the single place that knows the shape: it hands the UI the
 * legs, sides, fee and kind-specific facts it renders. Display only — every
 * rule lives in the backend.
 */

type Tagged<K extends TxKind> = Extract<Transaction, Record<K, unknown>>

/** Read the body of one variant (does the narrowing a `switch` on a call result cannot). */
export function body<K extends TxKind>(tx: Transaction, key: K): Tagged<K>[K] {
  return (tx as Tagged<K>)[key]
}

/** Every kind, in the order the UI lists them: the four verbs first. */
export const TX_KINDS: TxKind[] = [
  'topup',
  'payment',
  'transfer',
  'convert',
  'trade_spot',
  'trade_futures',
  'defi',
  'asset_sell',
  'adjustment',
]

export const TX_VERBS: TxKind[] = ['topup', 'payment', 'transfer', 'convert']

export function txKind(tx: Transaction): TxKind {
  return Object.keys(tx)[0] as TxKind
}

type AnyBody = { meta: TxMeta; fee: Fee | null }

function anyBody(tx: Transaction): AnyBody {
  return (tx as unknown as Record<TxKind, AnyBody>)[txKind(tx)]
}

export function txMeta(tx: Transaction): TxMeta {
  return anyBody(tx).meta
}

export function txId(tx: Transaction): string {
  return txMeta(tx).id
}

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

/** The two ends of a movement, when the kind has them. */
export function txSides(tx: Transaction): { from: AccountRef | null; to: AccountRef | null } {
  switch (txKind(tx)) {
    case 'topup':
      return { from: body(tx, 'topup').from, to: null }
    case 'payment':
      return { from: null, to: body(tx, 'payment').to }
    case 'transfer': {
      const record = body(tx, 'transfer')
      return { from: record.from, to: record.to }
    }
    case 'convert': {
      const record = body(tx, 'convert')
      return { from: record.from.account, to: record.to.account }
    }
    default:
      return { from: null, to: null }
  }
}

/** Every contact the transaction names. */
export function txContactIds(tx: Transaction): string[] {
  const { from, to } = txSides(tx)
  const ids = [txMeta(tx).contact_id, accountContactId(from), accountContactId(to)]
  return [...new Set(ids.filter((id): id is string => Boolean(id)))]
}

/**
 * A transfer or conversion between two stores of one top-level wallet. Mirrors
 * `Transaction::is_internal`; `rootOf` maps a store id to its root's id.
 */
export function txIsInternal(tx: Transaction, rootOf: (id: string) => string): boolean {
  const kind = txKind(tx)
  if (kind !== 'transfer' && kind !== 'convert') return false
  const { from, to } = txSides(tx)
  const left = accountStoreId(from)
  const right = accountStoreId(to)
  return Boolean(left && right && rootOf(left) === rootOf(right))
}

/** Whether no principal leg carries a price yet (the list marks it). */
export function txHasNoPrice(tx: Transaction): boolean {
  const legs = txPrincipalLegs(tx).filter((leg) => !leg.asset.contract?.startsWith('perp:'))
  return legs.length > 0 && legs.every((leg) => !leg.price_usd)
}

/** Chain and hash for the explorer link, when the variant has them. */
export function txChainAndHash(tx: Transaction): { chain: Chain | null; hash: string | null } {
  switch (txKind(tx)) {
    case 'topup': {
      const { details } = body(tx, 'topup')
      return { chain: details.network, hash: details.tx_hash }
    }
    case 'payment': {
      const { details } = body(tx, 'payment')
      return { chain: details.network, hash: details.tx_hash }
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      return { chain: record.network, hash: record.tx_hash }
    }
    case 'defi': {
      const record = body(tx, 'defi')
      return { chain: record.network, hash: record.tx_hash }
    }
    default:
      return { chain: null, hash: null }
  }
}

const EXPLORERS: Partial<Record<Chain, (hash: string) => string>> = {
  bitcoin: (hash) => `https://mempool.space/tx/${hash}`,
  ethereum: (hash) => `https://etherscan.io/tx/${hash}`,
  bsc: (hash) => `https://bscscan.com/tx/${hash}`,
  polygon: (hash) => `https://polygonscan.com/tx/${hash}`,
  arbitrum: (hash) => `https://arbiscan.io/tx/${hash}`,
  optimism: (hash) => `https://optimistic.etherscan.io/tx/${hash}`,
  base: (hash) => `https://basescan.org/tx/${hash}`,
  avalanche: (hash) => `https://snowtrace.io/tx/${hash}`,
  solana: (hash) => `https://solscan.io/tx/${hash}`,
  tron: (hash) => `https://tronscan.org/#/transaction/${hash}`,
}

/** Explorer page of the on-chain hash; mirrors `Transaction::explorer_url`. */
export function txExplorerUrl(tx: Transaction): string | null {
  const { chain, hash } = txChainAndHash(tx)
  if (!chain || !hash) return null
  return EXPLORERS[chain]?.(hash) ?? null
}

/** Kind-specific facts worth showing under a transaction, as `[i18n key, value]`. */
export function txDetails(tx: Transaction): Array<[string, string]> {
  const out: Array<[string, string]> = []
  switch (txKind(tx)) {
    case 'topup': {
      const { details } = body(tx, 'topup')
      if (details.unit_price) out.push(['tx.field.unitPrice', details.unit_price])
      break
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      if (record.network) out.push(['tx.field.network', record.network])
      break
    }
    case 'convert': {
      const record = body(tx, 'convert')
      if (record.rate_override) out.push(['tx.field.rate', record.rate_override])
      break
    }
    case 'trade_spot': {
      const record = body(tx, 'trade_spot')
      if (record.liquidity) out.push(['tx.field.liquidity', record.liquidity])
      if (record.order_id) out.push(['tx.field.order', record.order_id])
      break
    }
    case 'trade_futures': {
      const record = body(tx, 'trade_futures')
      out.push(['tx.field.position', `${record.direction} · ${record.action}`])
      out.push(['tx.field.price', record.fill_price])
      if (record.leverage) out.push(['tx.field.leverage', `${record.leverage}×`])
      break
    }
    case 'defi': {
      const record = body(tx, 'defi')
      out.push(['tx.field.action', record.action])
      if (record.protocol) out.push(['tx.field.protocol', record.protocol])
      if (record.pool_id) out.push(['tx.field.pool', record.pool_id])
      break
    }
    case 'asset_sell': {
      const record = body(tx, 'asset_sell')
      out.push(['tx.field.unitPrice', `${record.unit_price} ${record.currency.symbol}`])
      break
    }
    case 'adjustment': {
      for (const line of body(tx, 'adjustment').lines) {
        out.push(['adjustment.line', `${line.asset.symbol}: ${line.before} → ${line.after}`])
      }
      break
    }
    default:
      break
  }
  return out
}

/** The fee recorded on the transaction, with the asset it was paid in. */
export function txFee(tx: Transaction): { amount: string; asset: string } | null {
  const fee = anyBody(tx).fee
  if (!fee || Number(fee.amount) === 0) return null
  return { amount: fee.amount, asset: fee.asset.symbol }
}

/** Debt effect of a transfer, if it is one. */
export function txDebt(tx: Transaction): DebtEffect | null {
  return txKind(tx) === 'transfer' ? body(tx, 'transfer').debt : null
}

export type TxBadgeVariant = 'default' | 'secondary' | 'destructive' | 'outline'

/** Badge tone per kind (shared by the transaction list and a store's history). */
export const TX_KIND_BADGE: Record<TxKind, TxBadgeVariant> = {
  topup: 'default',
  payment: 'destructive',
  transfer: 'secondary',
  convert: 'secondary',
  trade_spot: 'secondary',
  trade_futures: 'secondary',
  defi: 'secondary',
  asset_sell: 'secondary',
  adjustment: 'outline',
}

export const DEFAULT_TX_BADGE: TxBadgeVariant = 'secondary'

/** Colour of a signed amount: green in, red out, neutral for zero or none. */
export function amountClass(signed: string): string {
  if (signed.startsWith('-')) return 'text-destructive'
  return Number(signed) > 0 ? 'text-emerald-600 dark:text-emerald-400' : ''
}

/** Colour for a whole transaction, from the kinds that only ever move money one way. */
export function txAmountClass(kind: TxKind): string {
  if (kind === 'topup') return 'text-emerald-600 dark:text-emerald-400'
  return kind === 'payment' ? 'text-destructive' : ''
}

/** Everything a free-text search should match: assets, comment, categories, hash, facts. */
export function txSearchText(tx: Transaction): string {
  const meta = txMeta(tx)
  const sides = txSides(tx)
  const raw = [sides.from, sides.to].map((side) => (side && 'raw' in side ? side.raw : ''))
  return [
    ...txLegs(tx).map((leg) => leg.asset.symbol),
    meta.comment,
    ...meta.categories,
    ...raw,
    txChainAndHash(tx).hash ?? '',
    ...txDetails(tx).map(([, value]) => value),
  ]
    .join(' ')
    .toLowerCase()
}
