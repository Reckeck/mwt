export interface ConfirmDialogProps {
  title: string
  /** What happens, in plain words: the consequence, not «Are you sure?». */
  description: string
  /** The red button; «Удалить» when omitted. */
  confirmLabel?: string
  /** The action is running: the confirm button spins and stays disabled. */
  pending?: boolean
  /** Name of the layer for `data-overlay`. */
  overlay?: string
}
