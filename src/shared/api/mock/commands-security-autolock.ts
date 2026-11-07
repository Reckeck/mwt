/** Commands: autolock. Not implemented yet, like the Rust side: every one answers `UNSUPPORTED`. */

import type { MockCommands } from './core'

const unsupported = () => Promise.reject({ code: 'UNSUPPORTED', message: 'unsupported vault file/version' })

export const securityAutolockCommands: MockCommands = {
  set_autolock: unsupported,
  autolock_activity: unsupported,
}
