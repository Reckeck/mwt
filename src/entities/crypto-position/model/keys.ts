/**
 * Query keys of the DeFi positions of a followed address (stage 2, spec E0).
 *
 * The key is the one shared list of this slice: a prefix from general to specific, so that a
 * mutation of spec D0 invalidates by `CRYPTO_POSITIONS_KEY` and reaches every query of the slice. The
 * queries and the components themselves come with spec D0.
 */

export const CRYPTO_POSITIONS_KEY = ['crypto', 'positions'] as const
