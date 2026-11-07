/** Commands: the security state. The mock keeps its level in memory; the raise moves it to `crypto`. */

import { resolve, type MockCommands } from './core'
import { mockAppSettings, mockPassword } from './vault-data'

/** The level the mock vault was raised to; `null` follows the password (a PIN or none). */
export const mockSecurityLevel: { value: SecurityLevel | null } = { value: null }

const DEFAULT_AUTOLOCK: AutolockSettings = { enabled: true, minutes: 15 }

export const securityStateCommands: MockCommands = {
  security_state: () => {
    const level: SecurityLevel = mockSecurityLevel.value ?? (mockPassword.value === null ? 'ledger' : 'pin')
    const state: SecurityState = {
      level,
      min_allowed_level: level,
      has_device_keys: false,
      autolock: mockAppSettings.autolock ?? DEFAULT_AUTOLOCK,
      biometric: 'unavailable',
      pin_retry_after_secs: null,
    }
    return resolve(state)
  },
}
