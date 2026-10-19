export interface PairInputProps {
  /** Label above the field; pass `''` to hide. */
  label?: string
  placeholder?: string
  /** Red `*` on the label: the form will not save without a pair. */
  required?: boolean
  /** Message under the field after a failed save (see `useFormValidation`). */
  error?: string
  /** `id` for the input, so an outer `<Label for>` can point at it. */
  id?: string
  /** Quote assets offered for a typed base, the first is the usual one. */
  quotes?: string[]
  /** `futures` shows «perpetual» next to the rows: a contract, not a spot pair. */
  kind?: 'spot' | 'futures'
}
