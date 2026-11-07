import type { Router } from 'vue-router'

/**
 * "Go to level 3" from the key form.
 *
 * TODO(H5B): open the raise-to-level-3 wizard here instead of the settings page; until it lands the
 * button takes the user to Settings, where the security level lives.
 */
export function openRaiseWizard(router: Router): void {
  void router.push('/settings')
}
