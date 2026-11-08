/** One stop of a tour: the element (by `data-tour`) and the i18n keys of its popover. */
export interface TourStep {
  target: string
  key: string
  side?: 'top' | 'right' | 'bottom' | 'left'
}
