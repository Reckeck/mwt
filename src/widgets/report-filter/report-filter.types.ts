/** What period the report asks for: a range of months, a year, or one month. */
export type ReportPeriod = 'range' | 'year' | 'month'

export interface ReportFilterBarProps {
  period: ReportPeriod
  /** Show the «in today's money» switch (flow and wealth). */
  realTerms?: boolean
  /** How many filters narrow the reports: «Сбросить (N)» shows when any. */
  active?: number
}
