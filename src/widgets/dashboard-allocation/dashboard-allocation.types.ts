/** Which breakdown of the portfolio the widget shows. */
export type AllocationAxis = 'chain' | 'store'

export interface DashboardAllocationProps {
  by: AllocationAxis
}
