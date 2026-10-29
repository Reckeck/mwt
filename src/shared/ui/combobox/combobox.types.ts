export interface ComboboxOption {
  value: string
  label: string
  /** Options with the same group render under one heading, in first-seen order. */
  group?: string
  groupLabel?: string
}

export interface ComboboxProps {
  options: ComboboxOption[]
  /** Several values, shown as removable chips inside the field. */
  multiple?: boolean
  id?: string
  placeholder?: string
  disabled?: boolean
  /** Shown when nothing matches and creation is off. */
  emptyLabel?: string
  /**
   * Enables creation: when the typed text matches no option exactly, a last
   * item with this label is offered and picking it (or Enter) emits `create`.
   */
  createLabel?: (text: string) => string
  /**
   * A second row next to the create one, for a lighter «just this name» choice
   * (a guest instead of a contact): picking it emits `guest` with the typed text.
   */
  guestLabel?: (text: string) => string
  /** Accessible name when there is no visible `<label for>`. */
  ariaLabel?: string
}
