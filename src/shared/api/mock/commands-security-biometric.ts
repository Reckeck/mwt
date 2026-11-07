/** Commands: biometric unlock. Not implemented yet, like the Rust side: every one answers `UNSUPPORTED`. */

import type { MockCommands } from './core'

const unsupported = () => Promise.reject({ code: 'UNSUPPORTED', message: 'unsupported vault file/version' })

export const securityBiometricCommands: MockCommands = {
  biometric_enable: unsupported,
  biometric_disable: unsupported,
  biometric_unlock: unsupported,
}
