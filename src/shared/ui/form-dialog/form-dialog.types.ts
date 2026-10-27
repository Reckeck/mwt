import type { DialogSize } from '@/shared/ui/dialog'

export interface FormDialogProps {
  open?: boolean
  title: string
  description?: string
  size?: DialogSize
  /** The `<form>` id; the footer's save button submits it, so it must be unique on the page. */
  formId: string
  /** The save button's text; "Сохранить" by default. */
  submitLabel?: string
  /** A save in flight: the only thing that disables the save button (rule 13). */
  pending?: boolean
  /** On a phone: no Cancel (the sheet has its ×), one full-width save button (Wallets-2 · 13, Wallets-3 · 20). */
  phoneSubmitOnly?: boolean
  /** Cancel's look: bare by default, `outline` where the frame draws a bordered button (Money-2 · 14, Money-3 · 17). */
  cancelVariant?: 'ghost' | 'outline'
  /** Name of the layer for `data-overlay`. */
  overlay?: string
}
