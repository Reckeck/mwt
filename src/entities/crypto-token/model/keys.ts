/**
 * Query keys of the tokens the crypto module met, quarantine included (stage 2, spec E0).
 *
 * The key is the one shared list of this slice: a prefix from general to specific, so that a
 * mutation of spec E2 invalidates by `CRYPTO_TOKENS_KEY` and reaches every query of the slice. The
 * queries and the components themselves come with spec E2.
 */

export const CRYPTO_TOKENS_KEY = ['crypto', 'tokens'] as const
