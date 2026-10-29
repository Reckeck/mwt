import type { SettleKind } from '@/entities/shared-event'

export interface SharedSettleDialogProps {
  event: SharedEventView
  /** Whose debt is settled; `null` while the dialog is closed. */
  person: PersonView | null
  /** `received` / `gave` by the sign of the debt, `offset` for a set-off without an operation. */
  kind: Exclude<SettleKind, 'paid'>
  /** Money of mine moves (a wallet is chosen); false for a guest, a set-off or someone else's event. */
  withWallet: boolean
}
