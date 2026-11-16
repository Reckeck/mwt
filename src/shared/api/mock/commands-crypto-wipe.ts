/**
 * Commands: wiping the module out of the vault.
 *
 * Owner: spec E12, with `service/cryptotrack/wipe.rs`. The mock keeps the rule the backend keeps:
 * a wipe deletes no wallet — a connected address loses its link to the module and goes to the
 * archive with its history — and the counts it reports are the ones it then clears, so the screen
 * in `pnpm dev` behaves like the application. The browser has no vault file, so the backup the
 * backend takes and reads back is reported as a name.
 */

import { type MockCommands, resolve } from './core'
import { mockCryptoTokens } from './commands-crypto-tokens'
import { mockStores } from './demo'

/** The wallets the module leads: a `crypto` wallet carrying `crypto_account_id`. */
const ledWallets = () => mockStores.filter((store) => !!store.crypto_account_id)

/**
 * What the browser session holds of the module. Events and the rest have no mock state of their
 * own, so a session that follows an address shows demo numbers of the right order.
 */
function counts(): CryptoWipeCounts {
  const accounts = ledWallets().length
  const following = accounts > 0
  return {
    accounts,
    events: following ? 24 : 0,
    overrides: following ? 2 : 0,
    tokens: mockCryptoTokens.length,
    positions: 0,
    snapshots: following ? 3 : 0,
    cursors: following ? accounts : 0,
    prices: following ? 12 : 0,
  }
}

const empty = (value: CryptoWipeCounts) => Object.values(value).every((count) => count === 0)

function preview(): CryptoWipePreview {
  const value = counts()
  const wallets = ledWallets().map((store) => ({ store_id: store.id, name: store.name }))
  return {
    counts: value,
    wallets,
    base_currency: 'rub',
    // Negative: what the module holds leaves the totals with it.
    value_delta: wallets.length ? '-412300' : '0',
    backup_at: null,
    device_secrets: false,
    empty: empty(value) && wallets.length === 0,
  }
}

export const cryptoWipeCommands: MockCommands = {
  crypto_wipe_preview: () => resolve(preview()),
  crypto_wipe_module: (call) => {
    const confirm = (call.body?.input as CryptoWipeInput | undefined)?.confirm
    if (confirm !== 'WIPE')
      return Promise.reject({ code: 'INVALID_INPUT', message: 'the wipe of the crypto module was not confirmed' })
    const before = preview()
    if (before.empty)
      return resolve<CryptoWipeReport>({
        counts: before.counts,
        wallets: [],
        base_currency: before.base_currency,
        value_delta: '0',
        backup: null,
        device_secrets_removed: false,
        wiped_at: null,
        can_lower_level: true,
      })
    // Nothing is deleted: the link goes and the wallet is archived, as in the vault.
    for (const store of ledWallets()) {
      store.crypto_account_id = undefined
      store.archived = true
    }
    mockCryptoTokens.length = 0
    return resolve<CryptoWipeReport>({
      counts: before.counts,
      wallets: before.wallets,
      base_currency: before.base_currency,
      value_delta: before.value_delta,
      backup: `vault-${new Date().toISOString().slice(0, 10)}.wallet`,
      device_secrets_removed: false,
      wiped_at: new Date().toISOString(),
      can_lower_level: true,
    })
  },
}
