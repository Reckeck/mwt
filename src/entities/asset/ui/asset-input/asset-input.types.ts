export interface AssetInputProps {
  /** Label rendered under the input; pass `''` to hide. */
  label?: string
  placeholder?: string
  invalid?: boolean
  /** Red `*` on the label: the form will not save without an asset. */
  required?: boolean
  /** Message under the field after a failed save (see `useFormValidation`). */
  error?: string
  /** Open the list when the field gets focus; off so a dialog's autofocus does not cover the form. */
  openOnFocus?: boolean
}
