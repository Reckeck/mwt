/**
 * Commands: biometric unlock (H7A). The mock has no OS, so `?biometric=` picks what the
 * "device" offers: `available`, `enabled`, `expired` (the 7-day window is over); without it
 * the platform is `unavailable`, as on Linux. Quick unlock exists at level 3 only.
 */

import { resolve, type MockCommands } from './core'
import { mockInfo, mockPassword, mockStatus } from './vault-data'

const PRESETS: Record<string, BiometricState> = {
  available: 'available',
  enabled: 'enabled',
  expired: 'password_required',
}
const preset = new URLSearchParams(globalThis.location?.search ?? '').get('biometric')
let current: BiometricState = (preset && PRESETS[preset]) || 'unavailable'

const fail = (code: string, message: string) => Promise.reject({ code, message })

export const mockBiometric = {
  /** What the device offers right now. */
  state: () => current,
  /** For the e2e scenarios: they cannot pick a URL parameter after the page has loaded. */
  set: (next: BiometricState) => {
    current = next
  },
}

export const securityBiometricCommands: MockCommands = {
  biometric_state: () => resolve(current),
  biometric_enable: ({ body }) => {
    if (current === 'unavailable') return fail('BIOMETRIC_UNAVAILABLE', 'no hardware-bound storage')
    if (body?.password !== mockPassword.value) return fail('SECRET', 'Wrong password')
    current = 'enabled'
    return resolve(current)
  },
  biometric_disable: () => {
    if (current !== 'unavailable') current = 'available'
    return resolve(current)
  },
  biometric_unlock: ({ body }) => {
    if (current === 'unavailable') return fail('BIOMETRIC_UNAVAILABLE', 'no hardware-bound storage')
    if (current === 'password_required') return fail('BIOMETRIC_PASSWORD_REQUIRED', 'the password is needed')
    mockStatus.value = {
      path: String(body?.path ?? mockStatus.value.path),
      exists: true,
      unlocked: true,
      info: mockInfo,
      storage: mockStatus.value.storage,
      encrypted: mockInfo.encrypted,
      sandbox: false,
    }
    return resolve(mockInfo)
  },
}
