/** Commands: autolock. The mock keeps the settings in the app settings, as the backend does. */

import { resolve, type MockCommands } from './core'
import { mockAppSettings } from './vault-data'

const MIN_MINUTES = 5
const MAX_MINUTES = 60

const invalid = (message: string) => Promise.reject({ code: 'INVALID_INPUT', message })

export const securityAutolockCommands: MockCommands = {
  set_autolock: ({ body }) => {
    const input = body as AutolockSettings
    if (!Number.isInteger(input.minutes) || input.minutes < MIN_MINUTES || input.minutes > MAX_MINUTES)
      return invalid(`autolock minutes must be ${MIN_MINUTES}-${MAX_MINUTES}`)
    // The mock device holds no exchange keys, so the lock may be off.
    mockAppSettings.autolock = { enabled: input.enabled, minutes: input.minutes }
    return resolve(mockAppSettings.autolock)
  },
  autolock_activity: () => resolve(null),
  autolock_hold_for_picker: () => resolve(null),
}
