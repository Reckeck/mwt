/**
 * Commands: the exchange key state.
 *
 * The browser mock keeps its vault at level 3, so a stored key reads as read-only and a wallet
 * without one as missing; `mockKeyLevelRequired` turns on the below-level-3 answer for a by-hand check.
 */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockKeyStatus } from './exchange-data'

/** Answer as a vault below level 3 would: every wallet `level_required`. */
export const mockKeyLevelRequired: { value: boolean } = { value: false }

export const securityKeysCommands: MockCommands = {
  read_exchange_key_state: (_call) => {
    const state: ExchangeKeyState = mockKeyLevelRequired.value
      ? 'level_required'
      : mockKeyStatus.value.configured
        ? 'read_only'
        : 'missing'
    return resolve(state)
  },
}
