export interface ContactArchiveProps {
  contact: Contact
}

export interface ContactEditDialogProps {
  /** The contact to change; none creates a new one. */
  contact?: Contact | null
}

export interface ContactAccountRowProps {
  account: ContactAccount
  /** «Записать перевод сюда» is off for an archived contact or without a wallet. */
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
  /** Why «Записать перевод сюда» cannot run (no wallet yet), or `true` to just disable it. */
  transferDisabled?: boolean | string
}
