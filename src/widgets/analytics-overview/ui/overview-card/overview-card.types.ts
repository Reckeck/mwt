import type { RouteLocationRaw } from 'vue-router'

export interface OverviewCardProps {
  title: string
  /** What period the figures cover, under the title. */
  period: string
  /** The detailed report. */
  to: RouteLocationRaw
  hasData: boolean
  insights: Insight[]
  /** Lowercase code the amounts are in. */
  currency: string
  /** The first step when there is nothing yet. */
  emptyTitle: string
  emptyDescription: string
}
