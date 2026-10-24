export interface NewPasswordFieldsProps {
  /** Prefix of the field ids (two forms may sit on a page). */
  id: string
  label?: string
  /** Message under the first field, under the second. */
  error?: string
  repeatError?: string
}
