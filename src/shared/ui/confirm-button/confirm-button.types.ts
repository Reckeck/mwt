export interface ConfirmButtonProps {
  /** Accessible name of the idle button (it usually shows only an icon). */
  label: string
  /** Text of the armed state, e.g. "Delete?". */
  confirmLabel: string
  disabled?: boolean
  /** How long the armed state waits for the second click, ms. */
  timeout?: number
}
