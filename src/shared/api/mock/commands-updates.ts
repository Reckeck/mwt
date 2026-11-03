/** Commands: app updates. In the browser every check steps through up to date, available, ready. */

import { emitMock } from '../mock-bus'
import { resolve } from './core'
import type { MockCommands } from './core'

const CYCLE: UpdatePhase[] = [
  { type: 'up_to_date' },
  {
    type: 'available',
    offer: {
      version: '9.9.9',
      notes: 'Mock release notes.',
      release_url: 'https://example.test/release',
      download_url: null,
      can_self_update: false,
      hint: 'release_page',
    },
  },
  { type: 'ready', version: '9.9.9' },
]
let step = 0
let lastCheck = new Date().toISOString()

function status(): UpdateStatusDto {
  return {
    phase: CYCLE[step % CYCLE.length] as UpdatePhase,
    current_version: '0.0.0-mock',
    install_kind: 'unknown',
    channel: 'dev',
    auto_update: true,
    last_check: lastCheck,
  }
}

export const updatesCommands: MockCommands = {
  update_status: () => resolve(status()),
  update_check_now: () => {
    step += 1
    lastCheck = new Date().toISOString()
    const next = status()
    setTimeout(() => emitMock('update-state', next), 0)
    return resolve(next)
  },
  update_restart_now: () => resolve(null),
}
