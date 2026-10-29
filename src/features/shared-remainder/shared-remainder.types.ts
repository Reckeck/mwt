export interface SharedRemainderDialogProps {
  event: SharedEventView
  /** Whose debt is left; `null` while the dialog is closed. */
  person: PersonView | null
  /** `write_off` makes the rest my expense; `reassign` splits it among the others. */
  kind: 'write_off' | 'reassign'
}

export interface SharedDueDialogProps {
  event: SharedEventView
  person: PersonView | null
}
