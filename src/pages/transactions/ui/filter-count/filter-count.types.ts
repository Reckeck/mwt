export interface FilterCountProps {
  filter: TxFilter
  /** Operations the filter keeps, all pages. */
  total: number
  /** `3 payments`: the filter keeps payments alone. */
  payments?: boolean
}
