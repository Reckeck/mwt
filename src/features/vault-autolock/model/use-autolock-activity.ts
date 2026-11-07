import { useEventListener } from '@vueuse/core'

import { call } from '@/shared/api/ipc'
import { useVaultStore } from '@/shared/store/vault'

import { createActivityThrottle } from '../lib/autolock'

/** What counts as the user being here (a finger or a mouse, a key, a wheel). */
const ACTIVITY_EVENTS = ['pointerdown', 'pointermove', 'keydown', 'wheel', 'touchstart'] as const

/**
 * Tells the backend the user is active, at most once per 30 s. The timer that
 * locks runs in Rust, so a throttled or frozen window cannot hold the lock off
 * or fire it early. Listeners go away with the scope (`useEventListener`).
 */
export function useAutolockActivity() {
  const vault = useVaultStore()
  const shouldSend = createActivityThrottle(() => Date.now())
  function report() {
    if (vault.locked || !shouldSend()) return
    call('autolock_activity').catch(() => undefined)
  }
  for (const name of ACTIVITY_EVENTS) useEventListener(window, name, report, { passive: true })
}
