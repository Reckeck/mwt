/** What period the report asks for: a range of months, a year, or one month. */
export type ReportPeriod = 'range' | 'year' | 'month'

export interface ReportFilterBarProps {
  period: ReportPeriod
  /** The wallet filter applies; the year report and inflation ignore it. */
  scoped?: boolean
  /** Offer «in today's money» under «Ещё фильтры». */
  realTerms?: boolean
  /** How many filters narrow the reports: «Сбросить (N)» shows when any. */
  active?: number
}
