/** What period the report asks for: a range of months, a year, or one month. */
export type ReportPeriod = 'range' | 'year' | 'month'

export interface ReportFilterBarProps {
  period: ReportPeriod
  /** Show the «in today's money» switch (flow and wealth). */
  realTerms?: boolean
  /** Show the wallets and currency filters; the running balance counts every wallet. */
  wallets?: boolean
  /** How many filters narrow the reports: «Сбросить (N)» shows when any. */
  active?: number
}
