/**
 * Reading a `Transaction` without matching on it everywhere.
 *
 * The backend `Transaction` is a closed union — one key per kind, each variant
 * carrying only the fields that kind has:
 *
 * ```ts
 * type Transaction =
 *   | { income: IncomeTx }
 *   | { bank_transfer: BankTransferTx }
 *   | ...
 * ```
 *
 * That is what makes an impossible record unrepresentable on the frontend too,
 * but it also means `tx.amount` no longer type-checks. Components should not
 * each write their own narrowing chain: this module is the single place that
 * knows the shape, and it hands the UI a flat "row" plus the kind-specific
 * extras it actually renders.
 */

/**
 * Read the body of one variant.
 *
 * `switch (txTag(tx))` does not narrow `tx` — TypeScript cannot correlate a
 * value returned by a function call with the object it came from. This does the
 * narrowing once, in one place, so every caller gets a correctly typed body
 * without a cast of its own.
 */
type Tagged<K extends TxTag> = Extract<Transaction, Record<K, unknown>>

export function body<K extends TxTag>(tx: Transaction, key: K): Tagged<K>[K] {
  return (tx as Tagged<K>)[key]
}

/**
 * The kinds a wallet of one kind accepts, in the two groups the UI shows.
 *
 * Mirrors `Transaction::tags_for_store_kind` on the Rust side. This is a typed
 * wrapper only — the data comes from the `transaction_kinds_for_store_kind`
 * command, so there is no second copy of the binding to drift.
 */
export interface StoreKindKinds {
  common: TxTag[]
  specific: TxTag[]
  default: TxTag
}

/** Narrow the backend's `string[]` to `TxTag[]`, dropping anything unknown. */
export function asTags(raw: string[]): TxTag[] {
  return raw.filter((tag): tag is TxTag => (TX_TAGS as string[]).includes(tag))
}

export function readStoreKindKinds(raw: TransactionKinds): StoreKindKinds {
  const common = asTags(raw.common)
  const specific = asTags(raw.specific)
  const fallback: TxTag = 'income'
  return {
    common,
    specific,
    // The backend's default is authoritative, but only if it is actually
    // offered — an unknown tag must not leave the picker on nothing.
    default: [...common, ...specific].includes(raw.default as TxTag)
      ? (raw.default as TxTag)
      : (specific[0] ?? common[0] ?? fallback),
  }
}

/** The externally tagged key of a transaction. */
export type TxTag =
  | 'income'
  | 'expense'
  | 'payment'
  | 'fee'
  | 'bank_transfer'
  | 'card_payment'
  | 'wallet_transfer'
  | 'defi_action'
  | 'exchange_trade'
  | 'transfer'
  | 'cash_flow'

/** Transaction kind tag, mirroring the Rust `Transaction::tags_for_store_kind`. */
export type TxKind = TxTag

/** Mirrors `trade_core::model::transaction::ALL_TAGS`. */
export const TX_TAGS: TxTag[] = [
  'income',
  'expense',
  'payment',
  'fee',
  'bank_transfer',
  'card_payment',
  'wallet_transfer',
  'defi_action',
  'exchange_trade',
  'transfer',
  'cash_flow',
]

/** One `(store, asset)` effect, mirroring the Rust `Leg`. */
export interface TxLegView {
  store_id: string
  asset: AssetRef
  /** Signed: positive adds to the store, negative subtracts. */
  amount: string
  to_store_id: string | null
  price_usd: string | null
}

export function txTag(tx: Transaction): TxTag {
  return Object.keys(tx)[0] as TxTag
}

/**
 * Common fields of any transaction.
 *
 * Every variant body carries a `meta`, so this needs no per-variant match —
 * but it cannot be expressed as `Extract<Transaction, Record<TxTag, unknown>>`
 * either, since no single union member has all eleven keys.
 */
export function txMeta(tx: Transaction): TxMeta {
  for (const key of TX_TAGS) {
    const record = (tx as Partial<Record<TxTag, { meta: TxMeta }>>)[key]
    if (record) return record.meta
  }
  // Unreachable for a well-formed union; keeps callers total rather than
  // throwing inside a render function.
  return { id: '', timestamp: '', comment: '', categories: [] }
}

export function txId(tx: Transaction): string {
  return txMeta(tx).id
}

/**
 * Expand a transaction into the movements it made.
 *
 * Mirrors `Transaction::legs()` on the backend so the UI never has to know
 * that a trade is "sold plus bought" while a bank transfer is one movement.
 */
export function txLegs(tx: Transaction): TxLegView[] {
  const leg = (
    store_id: string,
    asset: AssetRef,
    amount: string,
    price: PriceStamp | undefined,
    to_store_id: string | null = null,
  ): TxLegView => ({
    store_id,
    asset,
    amount,
    to_store_id,
    price_usd: price?.price_usd ?? null,
  })

  const tag = txTag(tx)
  switch (tag) {
    case 'income': {
      const record = body(tx, 'income')
      return [leg(record.store_id, record.asset, record.amount, record.price)]
    }
    case 'expense': {
      const record = body(tx, 'expense')
      return [leg(record.store_id, record.asset, `-${record.amount}`, record.price)]
    }
    case 'payment': {
      const record = body(tx, 'payment')
      return [leg(record.store_id, record.asset, `-${record.amount}`, record.price)]
    }
    case 'fee': {
      const record = body(tx, 'fee')
      return [leg(record.store_id, record.asset, `-${record.amount}`, record.price)]
    }
    case 'bank_transfer': {
      const record = body(tx, 'bank_transfer')
      const signed = record.flow === 'in' ? record.amount : `-${record.amount}`
      return [leg(record.store_id, record.asset, signed, record.price)]
    }
    case 'card_payment': {
      const record = body(tx, 'card_payment')
      return [leg(record.store_id, record.asset, `-${record.amount}`, record.price)]
    }
    case 'wallet_transfer': {
      const record = body(tx, 'wallet_transfer')
      const signed = record.flow === 'in' ? record.amount : `-${record.amount}`
      return [leg(record.store_id, record.asset, signed, record.price)]
    }
    case 'defi_action': {
      const record = body(tx, 'defi_action')
      const signed = record.flow === 'in' ? record.amount : `-${record.amount}`
      return [leg(record.store_id, record.asset, signed, record.price)]
    }
    case 'cash_flow': {
      const record = body(tx, 'cash_flow')
      const signed = record.flow === 'in' ? record.amount : `-${record.amount}`
      return [leg(record.store_id, record.asset, signed, record.price)]
    }
    case 'exchange_trade': {
      const record = body(tx, 'exchange_trade')
      return [
        leg(record.store_id, record.sold.asset, `-${record.sold.qty}`, record.price),
        leg(record.store_id, record.bought.asset, record.bought.qty, record.price),
      ]
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      return [
        leg(record.from_store_id, record.asset, `-${record.amount}`, record.price, record.to_store_id),
        leg(record.to_store_id, record.asset, record.amount, record.price, record.from_store_id),
      ]
    }
  }
}

/** The store the transaction is booked on — for a transfer, the source. */
export function txStoreId(tx: Transaction): string {
  for (const move of txLegs(tx)) return move.store_id
  return ''
}

/** The leg belonging to `store_id`, or the first one. */
export function txLegForStore(tx: Transaction, store_id: string): TxLegView {
  const moves = txLegs(tx)
  return moves.find((move) => move.store_id === store_id) ?? moves[0]
}

/** Chain and hash for the explorer link, when the variant has them. */
export function txChainAndHash(tx: Transaction): { chain: Chain | null; hash: string | null } {
  const tag = txTag(tx)
  if (tag === 'wallet_transfer') {
    const record = body(tx, 'wallet_transfer')
    return { chain: record.network ?? null, hash: record.tx_hash ?? null }
  }
  if (tag === 'defi_action') {
    const record = body(tx, 'defi_action')
    return { chain: record.network ?? null, hash: record.tx_hash ?? null }
  }
  if (tag === 'transfer') {
    const record = body(tx, 'transfer')
    return { chain: record.network ?? null, hash: record.tx_hash ?? null }
  }
  return { chain: null, hash: null }
}

/**
 * Kind-specific facts worth showing under a transaction.
 *
 * Returned as a list of label/value strings rather than a typed bag, because
 * which facts exist is exactly what differs per kind and the UI only ever
 * renders them as text.
 */
export function txDetails(tx: Transaction): Array<[string, string]> {
  const tag = txTag(tx)
  switch (tag) {
    case 'bank_transfer': {
      const record = body(tx, 'bank_transfer')
      const out: Array<[string, string]> = [['tx.detail.rail', record.rail]]
      if (record.counterparty_name) out.push(['tx.detail.counterparty', record.counterparty_name])
      if (record.counterparty_account) out.push(['tx.detail.account', record.counterparty_account])
      if (record.reference) out.push(['tx.detail.reference', record.reference])
      return out
    }
    case 'card_payment': {
      const record = body(tx, 'card_payment')
      const out: Array<[string, string]> = []
      if (record.merchant) out.push(['tx.detail.merchant', record.merchant])
      if (record.card_last4) out.push(['tx.detail.card', `•••• ${record.card_last4}`])
      if (record.mcc) out.push(['tx.detail.mcc', record.mcc])
      return out
    }
    case 'payment': {
      const record = body(tx, 'payment')
      const out: Array<[string, string]> = []
      if (record.payee) out.push(['tx.detail.payee', record.payee])
      if (record.receipt_ref) out.push(['tx.detail.reference', record.receipt_ref])
      return out
    }
    case 'wallet_transfer': {
      const record = body(tx, 'wallet_transfer')
      const out: Array<[string, string]> = []
      if (record.to_address) out.push(['tx.detail.to', record.to_address])
      if (record.from_address) out.push(['tx.detail.from', record.from_address])
      if (record.memo) out.push(['tx.detail.memo', record.memo])
      return out
    }
    case 'defi_action': {
      const record = body(tx, 'defi_action')
      const out: Array<[string, string]> = [['tx.detail.action', record.action]]
      if (record.protocol) out.push(['tx.detail.protocol', record.protocol])
      if (record.pool_id) out.push(['tx.detail.pool', record.pool_id])
      return out
    }
    case 'exchange_trade': {
      const record = body(tx, 'exchange_trade')
      const out: Array<[string, string]> = []
      if (record.market) out.push(['tx.detail.market', record.market])
      if (record.liquidity) out.push(['tx.detail.liquidity', record.liquidity])
      if (record.order_id) out.push(['tx.detail.order', record.order_id])
      return out
    }
    case 'income': {
      const record = body(tx, 'income')
      return record.source === 'other' ? [] : [['tx.detail.source', record.source]]
    }
    case 'cash_flow': {
      const record = body(tx, 'cash_flow')
      const out: Array<[string, string]> = []
      if (record.location) out.push(['tx.detail.location', record.location])
      if (record.counterparty) out.push(['tx.detail.counterparty', record.counterparty])
      return out
    }
    default:
      return []
  }
}

/**
 * The IPC command that records a kind.
 *
 * One per kind because each takes its own input DTO; only the trade is named
 * after the action rather than the tag.
 */
export const TX_COMMAND: Record<TxTag, string> = {
  income: 'add_income',
  expense: 'add_expense',
  payment: 'add_payment',
  fee: 'add_fee',
  bank_transfer: 'add_bank_transfer',
  card_payment: 'add_card_payment',
  wallet_transfer: 'add_wallet_transfer',
  defi_action: 'add_defi_action',
  exchange_trade: 'add_trade',
  transfer: 'add_transfer',
  cash_flow: 'add_cash_flow',
}

/** Kinds whose money moves in a direction the user picks (`flow`). */
export const TX_TAGS_WITH_FLOW: ReadonlySet<TxTag> = new Set<TxTag>([
  'bank_transfer',
  'wallet_transfer',
  'defi_action',
  'cash_flow',
])

/** Kinds that record a fee of their own. */
export const TX_TAGS_WITH_FEE: ReadonlySet<TxTag> = new Set<TxTag>([
  'bank_transfer',
  'card_payment',
  'wallet_transfer',
  'defi_action',
  'exchange_trade',
  'transfer',
])

/** Kinds that carry a network and an on-chain hash. */
export const TX_TAGS_ON_CHAIN: ReadonlySet<TxTag> = new Set<TxTag>(['wallet_transfer', 'defi_action', 'transfer'])

export type TxBadgeVariant = 'default' | 'secondary' | 'destructive' | 'outline'

/**
 * Badge tone per kind.
 *
 * Lives here rather than in an entity: both the transaction list and a store's
 * history render it, and entities must not import each other.
 */
export const TX_KIND_BADGE: Record<TxTag, TxBadgeVariant> = {
  income: 'default',
  expense: 'destructive',
  payment: 'destructive',
  transfer: 'secondary',
  exchange_trade: 'secondary',
  fee: 'outline',
  bank_transfer: 'secondary',
  card_payment: 'destructive',
  wallet_transfer: 'secondary',
  defi_action: 'secondary',
  cash_flow: 'secondary',
}

export const DEFAULT_TX_BADGE: TxBadgeVariant = 'secondary'

/** Colour of a signed amount: green in, red out, neutral for zero or none. */
export function amountClass(signed: string): string {
  if (signed.startsWith('-')) return 'text-destructive'
  return Number(signed) > 0 ? 'text-emerald-600 dark:text-emerald-400' : ''
}

/** Colour for a whole transaction, from the kinds that only ever move money one way. */
export function txAmountClass(tag: TxTag): string {
  if (tag === 'income') return 'text-emerald-600 dark:text-emerald-400'
  return tag === 'expense' || tag === 'payment' || tag === 'fee' || tag === 'card_payment' ? 'text-destructive' : ''
}

/** The fee recorded on the transaction itself, with the asset it was paid in. */
export function txFee(tx: Transaction): { amount: string; asset: string } | null {
  const tag = txTag(tx)
  if (!TX_TAGS_WITH_FEE.has(tag)) return null
  // Every kind in TX_TAGS_WITH_FEE has `fee` and `fee_asset`; same lookup as `txMeta`.
  const record = (tx as Partial<Record<TxTag, { fee: string | null; fee_asset: string | null }>>)[tag]
  if (!record?.fee) return null
  return { amount: record.fee, asset: record.fee_asset ?? txLegs(tx)[0]?.asset.symbol ?? '' }
}

/** Everything a free-text search should match: assets, comment, tags, hash, kind-specific facts. */
export function txSearchText(tx: Transaction): string {
  const meta = txMeta(tx)
  return [
    ...txLegs(tx).map((move) => move.asset.symbol),
    meta.comment,
    ...meta.categories,
    txChainAndHash(tx).hash ?? '',
    ...txDetails(tx).map(([, value]) => value),
  ]
    .join(' ')
    .toLowerCase()
}
