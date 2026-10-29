import type { SignTone } from '@/shared/lib/format'

export interface StatCardProps {
  /** What the figure is ("Итого", "Реализованный P&L"); the `label` slot swaps in a label with an explanation. */
  label: string
  /** The figure as text; or put `<Money>`/`<PnlText>` in the default slot instead. */
  value?: string
  /** One muted line under the figure. */
  hint?: string
  /** Colours a plain-text `value` by direction. */
  tone?: SignTone
  /** Shows a placeholder instead of a figure that would read as zero. */
  loading?: boolean
}
