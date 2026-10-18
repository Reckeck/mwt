export interface ReportInsightsProps {
  /** Facts from the backend, the most important first. */
  insights: Insight[]
  /** Lowercase code the amounts are in. */
  currency: string
  /** Only the first fact, in one line: for a card. */
  compact?: boolean
}
