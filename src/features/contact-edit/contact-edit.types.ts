export interface ContactArchiveProps {
  contact: Contact
}

export interface ContactEditDialogProps {
  /** The contact to change; none creates a new one. */
  contact?: Contact | null
  /** A name to start a new contact from (what was typed in a picker). */
  initialName?: string
}

export interface ContactAccountRowProps {
  account: ContactAccount
  /** `Record a transfer here` is off for an archived contact or without a wallet. */
  transferDisabled?: boolean | string
}

export interface ContactAccountFormProps {
  /** The requisite being changed; none adds a new one. */
  initial?: ContactAccount | null
  /** A pasted value to start from. */
  value?: string
  /** Groups already in use on this contact's requisites. */
  groups: string[]
  pending?: boolean
}

export interface ContactAccountsProps {
  contact: Contact
  /** Why `Record a transfer here` cannot run (no wallet yet), or `true` to just disable it. */
  transferDisabled?: boolean | string
}
