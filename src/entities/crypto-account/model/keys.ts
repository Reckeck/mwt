/**
 * Query keys of the addresses and exchanges the crypto module follows (stage 2, spec E0).
 *
 * The key is the one shared list of this slice: a prefix from general to specific, so that a
 * mutation of spec E9 invalidates by `CRYPTO_ACCOUNTS_KEY` and reaches every query of the slice. The
 * queries and the components themselves come with spec E9.
 */

export const CRYPTO_ACCOUNTS_KEY = ['crypto', 'accounts'] as const

/** What the move to level 3 would retire from the totals (spec E11): one read-only answer, shared by the wizard and the tests. */
export const CRYPTO_TRANSITION_KEY = ['crypto', 'transition'] as const
