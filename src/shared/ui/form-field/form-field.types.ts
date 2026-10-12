export interface FormFieldProps {
  /** Text of the label; leave empty to render the label yourself. */
  label?: string
  /** `id` of the control, so the label points at it. */
  for?: string
  /** Shows the red `*`: the form will not save without this field. */
  required?: boolean
  /** Message under the field; the field turns red. Pass `form.error('name')`. */
  error?: string
  /** Hint under the field, hidden while an error is shown. */
  hint?: string
}
