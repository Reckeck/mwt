export interface FilterBarProps {
  /** How many filters narrow the list; «Сбросить (N)» shows when any. */
  active?: number
  /** How many of the folded filters are set: the «Ещё фильтры (N)» badge. */
  moreActive?: number
  /** `false`: only the lead field is there, so a phone gets no «Фильтры» sheet. */
  folds?: boolean
}
