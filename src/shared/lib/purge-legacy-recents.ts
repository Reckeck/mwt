/** Keys of the recents lists that older versions wrote to disk; they held vault titles and filters. */
const LEGACY_KEYS = ['palette.recent', 'trade-app:recent-places']

/** Removes the leftovers of old installs. Call once at startup, before mount. */
export function purgeLegacyRecents(): void {
  try {
    for (const key of LEGACY_KEYS) localStorage.removeItem(key)
  } catch {
    // Blocked storage: nothing was written there either.
  }
}
