/**
 * Commands: the exchange key state.
 * Not implemented yet, like the Rust side: every one answers `UNSUPPORTED`.
 */

import type { MockCommands } from './core'

const unsupported = () => Promise.reject({ code: 'UNSUPPORTED', message: 'unsupported vault file/version' })

export const securityKeysCommands: MockCommands = {
  read_exchange_key_state: unsupported,
}
