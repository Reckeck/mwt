import type { TourStep } from './tours.types.ts'

/**
 * Vault protection steps of the settings tour (Settings -> Vault): the level card, the way to level 3
 * from the exchange key form, and quick unlock. A step whose element is not on screen is skipped
 * (the raise button shows only while a key needs level 3; the toggle only where the OS offers it).
 */
export const SECURITY_TOUR_STEPS: TourStep[] = [
  { target: 'security-level', key: 'securityOnboarding.level', side: 'top' },
  { target: 'security-raise-start', key: 'securityOnboarding.raise', side: 'bottom' },
  { target: 'biometric-toggle', key: 'securityOnboarding.biometric', side: 'top' },
]

/** The `Level 3 required` refusal of sync (Settings -> Sync -> Devices): the banner on the refusing device, the note on the sender's row. */
export const SYNC_LEVEL_TOUR_STEPS: TourStep[] = [
  { target: 'sync-level-required', key: 'securityOnboarding.syncLevel', side: 'bottom' },
]
