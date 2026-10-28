export interface ContactPickerProps {
  id?: string
  placeholder?: string
  ariaLabel?: string
  /** Label of the «no contact» row; without it the field cannot be cleared. */
  emptyLabel?: string
  /** Pick only: no «Создать «Петя»» row (filters). */
  noCreate?: boolean
  disabled?: boolean
}
