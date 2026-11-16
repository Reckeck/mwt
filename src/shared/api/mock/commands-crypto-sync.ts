/**
 * Commands: the background tracking of the followed addresses.
 *
 * Owner: spec E6. The browser has no network of its own to read, so the mock keeps only what the
 * screen can see: the one-time consent, and a status with no accounts until the accounts mock
 * (spec E9) has some. Like the application, "read now" is refused without the consent.
 */

import { mockFollowedAccountIds } from './commands-crypto-accounts'
import { type MockCommands, resolve } from './core'

let consent: boolean | null = null

/** A read that finished a few minutes ago: the demo addresses hold what the engine brought them. */
const lastOkAt = () => new Date(Date.now() - 7 * 60_000).toISOString()

const status = (): CryptoSyncStatus => ({
  consent,
  running: false,
  runs_here: true,
  accounts: mockFollowedAccountIds().map((accountId) => ({
    account_id: accountId,
    state: 'done',
    last_ok_at: consent === true ? lastOkAt() : null,
    error: null,
    added: 0,
  })),
})

export const cryptoSyncCommands: MockCommands = {
  crypto_sync_status: () => resolve(status()),
  crypto_sync_now: () =>
    consent === true
      ? resolve(null)
      : Promise.reject({ code: 'INVALID_INPUT', message: 'reading the addresses is turned off' }),
  crypto_set_tracking_consent: ({ args }) => {
    consent = args?.consent === true
    return resolve(null)
  },
}
