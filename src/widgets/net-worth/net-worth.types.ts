/** How far back the capital line goes; the labels are `money.wealth.spans.*`. */
export type NetWorthSpan = 'last3' | 'last6' | 'last12' | 'all'

export interface NetWorthProps {
  points: NetworthPoint[]
  /** Currency of the report (lower-case code). */
  currency: string
  /** The report is on its way: the figure keeps its height. */
  loading?: boolean
}
