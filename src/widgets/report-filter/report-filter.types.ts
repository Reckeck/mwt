export interface ReportFilterBarProps {
  /** The report the filter is for: the year report asks for a year, not a range. */
  kind: ReportKind
  /** How many filters narrow the reports: «Сбросить (N)» shows when any. */
  active?: number
}
