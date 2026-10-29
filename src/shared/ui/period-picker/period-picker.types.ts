import type { MonthRange } from '@/shared/lib/period'

export interface PeriodPickerProps {
  /** What `Reset` returns to: the tab's own default period. */
  defaultRange: MonthRange
  /** The trigger's width on a wide screen, as on the canvas (170px). */
  triggerClass?: string
}
