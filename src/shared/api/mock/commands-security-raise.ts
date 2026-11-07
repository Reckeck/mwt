/** Commands: the move to the crypto level. Not implemented yet, like the Rust side: every one answers `UNSUPPORTED`. */

import type { MockCommands } from './core'

const unsupported = () => Promise.reject({ code: 'UNSUPPORTED', message: 'unsupported vault file/version' })

export const securityRaiseCommands: MockCommands = {
  raise_to_crypto_level: unsupported,
  validate_password_strength: unsupported,
}
