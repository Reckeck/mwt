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

export { body, TX_KINDS, TX_VERBS, txKind, txMeta, txId } from './access'
export type { TxLegView } from './legs'
export { accountStoreId, accountContactId, txLegs, txPrincipalLegs, txStoreId, txLegForStore } from './legs'
export * from './sides'
export * from './explorer'
export * from './pair'
export * from './facts'
export * from './presentation'
export * from './search'
