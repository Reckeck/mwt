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
}
