export interface CryptoWipeDialogProps {
  /** What the wipe would take, as the vault counted it. */
  preview: CryptoWipePreview
  /** The wipe is running: the confirm button spins and stays disabled. */
  pending: boolean
}
