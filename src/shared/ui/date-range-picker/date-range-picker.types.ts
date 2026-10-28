export interface DateRangePickerProps {
  /** Two ends (a trip, a weekend) instead of one day. */
  range?: boolean
  /** Shown while empty. */
  placeholder?: string
  /** Red border, as for a field with an error under it. */
  invalid?: boolean
  disabled?: boolean
}
