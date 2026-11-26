/**
 * What the crypto module's screens may reach, one source for both guards: the unit test beside
 * (`no-prefetch.test.ts`) and the static check (`tests/checks/crypto-readonly.check.ts`). Nothing imports it
 * at run time.
 */

/** The slices that show the module (spec E10). */
export const CRYPTO_MODULE_SLICES = [
  'src/entities/crypto-tx',
  'src/entities/crypto-account',
  'src/entities/crypto-token',
  'src/entities/crypto-position',
  'src/pages/crypto-positions',
  'src/widgets/price-chart',
] as const

/**
 * Everything these screens may ask the backend for. A command that would add or drop an operation or position
 * has to be added here first, and that is the review.
 */
export const CRYPTO_ALLOWED_CALLS = [
  'crypto_list_accounts',
  'crypto_sync_status',
  'crypto_list_tokens',
  'crypto_list_positions',
  // The user's verdict on a quarantined token: it decides whether the token counts, it adds or drops no operation.
  'crypto_confirm_token',
  'crypto_reject_token',
  'crypto_clear_token_verdict',
  // Market data, read only.
  'get_candles',
] as const
