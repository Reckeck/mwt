export interface ContactPickerProps {
  id?: string
  placeholder?: string
  ariaLabel?: string
  /** Label of the «no contact» row; without it the field cannot be cleared. */
  emptyLabel?: string
  /** Pick only: no `Create "Pete"` row (filters). */
  noCreate?: boolean
  disabled?: boolean
  /**
   * A second row `Add as a guest "Olga"` next to `Create`: picking it emits `guest`
   * with the typed name and picks nobody (events: a person without a contact).
   */
  guestLabel?: string
}
