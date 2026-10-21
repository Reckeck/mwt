/** Which breakdown of the portfolio the widget shows. */
export type AllocationAxis = 'store' | 'chain' | 'class'

export const ALLOCATION_AXES: readonly AllocationAxis[] = ['store', 'chain', 'class']

export interface DashboardAllocationProps {
  /** The breakdown it opens with; the widget switches it in place. */
  by?: AllocationAxis
}
