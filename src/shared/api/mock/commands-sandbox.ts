/** Commands: the sandbox (`commands::sandbox` in Rust), with state: see `sandbox-state.ts`. */

import { resolve } from './core'
import type { MockCommands } from './core'
import { isSandbox, leaveSandbox, openSandbox, resetSandbox } from './sandbox-state'

export const sandboxCommands: MockCommands = {
  open_sandbox: (_call) => resolve<VaultInfo>(openSandbox()),
  reset_sandbox: (_call) => resolve<VaultInfo>(resetSandbox()),
  close_sandbox: (_call) => {
    if (!leaveSandbox()) throw { code: 'SANDBOX', message: 'sandbox: the sandbox is not open' }
    return resolve(undefined)
  },
  is_sandbox: (_call) => resolve<boolean>(isSandbox()),
}
