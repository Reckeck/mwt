import { reactive } from 'vue'

/**
 * Where the move to level 3 stands, shared by whoever starts it (the exchange key form) and the
 * wizard itself. Nothing is stored between launches: an interrupted move starts again at step 2.
 */
const wizard = reactive({ open: false, interrupted: false })

/** Open the wizard; after an interruption it continues from the password step. */
export function openRaiseWizard(): void {
  wizard.open = true
}

export function useRaiseWizardState() {
  return wizard
}
