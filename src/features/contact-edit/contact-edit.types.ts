export interface ContactDialogProps {
  /** A contact to edit; `null` creates a new one. */
  contact?: Contact | null
}

export interface ContactArchiveProps {
  contact: Contact
}
