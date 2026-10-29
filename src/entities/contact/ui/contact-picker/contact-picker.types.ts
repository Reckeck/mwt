export interface ContactPickerProps {
  id?: string
  placeholder?: string
  ariaLabel?: string
  /** Label of the «no contact» row; without it the field cannot be cleared. */
  emptyLabel?: string
  /** Pick only: no «Создать «Петя»» row (filters). */
  noCreate?: boolean
  disabled?: boolean
  /**
   * A second row «Добавить гостем «Оля»» next to «Создать»: picking it emits `guest`
   * with the typed name and picks nobody (events: a person without a contact).
   */
  guestLabel?: string
}
