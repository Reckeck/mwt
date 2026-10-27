export interface ConfirmButtonProps {
  /** Accessible name of the idle button (it usually shows only an icon). */
  label: string
  /** The question in the confirmation, e.g. "Delete the transaction?". */
  confirmLabel: string
  /** The consequence under the question: "You can undo this from the notification." */
  description?: string
  /** Text of the red button; «Delete» when omitted. */
  actionLabel?: string
  disabled?: boolean
  /** Show the label beside the icon: an outlined button in a footer. */
  showLabel?: boolean
}
