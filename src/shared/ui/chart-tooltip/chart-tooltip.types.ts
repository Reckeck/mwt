import type { SignTone } from '@/shared/lib/format'

export interface ChartTooltipRow {
  label: string
  /** Already formatted. */
  value: string
  tone?: SignTone
}

export interface ChartTooltipProps {
  /** The point being read (a date, a category). */
  title: string
  rows: ChartTooltipRow[]
}
