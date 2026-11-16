/**
 * Query keys of the operations of the crypto module (stage 2, specs E0 and E10).
 *
 * A prefix from general to specific, so that a correction invalidates by `CRYPTO_OPERATIONS_KEY` and
 * reaches the list and every card. The list key carries the filter, so a new filter is a new key
 * (and keeps the previous rows on screen until the answer comes).
 */

export const CRYPTO_OPERATIONS_KEY = ['crypto', 'operations'] as const

export const cryptoOperationsListKey = (filter: CryptoOpFilter) => [...CRYPTO_OPERATIONS_KEY, 'list', filter] as const

export const cryptoOperationDetailKey = (id: string, withCandidates: boolean) =>
  [...CRYPTO_OPERATIONS_KEY, 'detail', id, withCandidates] as const
