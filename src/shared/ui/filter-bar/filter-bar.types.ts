export interface FilterBarProps {
  /** How many filters narrow the list; `Reset (N)` shows when any. */
  active?: number
  /** How many of the folded filters are set: the `More filters (N)` badge. */
  moreActive?: number
  /** `false`: only the lead field is there, so a phone gets no `Filters` screen. */
  folds?: boolean
  /** The panel's apply button: `Show 5`. */
  applyLabel?: string
  /** The same on a phone, where there is room: `Show 5 operations`. */
  applyLabelFull?: string
}
