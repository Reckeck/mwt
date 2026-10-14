/** Reaching into the union: the body of a variant, its kind, meta and id. */

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

export function anyBody(tx: Transaction): AnyBody {
  return (tx as unknown as Record<TxKind, AnyBody>)[txKind(tx)]
}

export function txMeta(tx: Transaction): TxMeta {
  return anyBody(tx).meta
}

export function txId(tx: Transaction): string {
  return txMeta(tx).id
}
