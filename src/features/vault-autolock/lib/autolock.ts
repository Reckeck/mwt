import { i18n } from '@/shared/config/i18n'

/** The range the backend accepts (`AUTOLOCK_MIN_MINUTES` / `AUTOLOCK_MAX_MINUTES`). */
export const AUTOLOCK_MIN = 5
export const AUTOLOCK_MAX = 60
export const AUTOLOCK_DEFAULT = 15

/** The window tells the backend about activity at most this often; the idle timer itself runs in Rust. */
export const ACTIVITY_INTERVAL_MS = 30_000

/** What a device that never chose gets; mirrors `default_autolock()` in `appconfig.rs`. */
export const DEFAULT_AUTOLOCK: AutolockSettings = { enabled: true, minutes: AUTOLOCK_DEFAULT }

/**
 * Whether an activity should be sent now: the first one, then one per interval.
 * `now` is injected so the rule is tested on a fake clock.
 */
export function createActivityThrottle(now: () => number, interval = ACTIVITY_INTERVAL_MS): () => boolean {
  let last: number | null = null
  return () => {
    const current = now()
    // A clock that went backwards sends too: better one extra call than a lock under a working user.
    if (last !== null && current >= last && current - last < interval) return false
    last = current
    return true
  }
}

/** The minutes field: blank or outside 5-60 gives the message for the field, else `null`. */
export function minutesError(value: string): string | null {
  const parsed = Number(value.trim())
  const ok = value.trim() !== '' && Number.isInteger(parsed) && parsed >= AUTOLOCK_MIN && parsed <= AUTOLOCK_MAX
  return ok ? null : i18n.global.t('autolock.rangeError', { min: AUTOLOCK_MIN, max: AUTOLOCK_MAX })
}
