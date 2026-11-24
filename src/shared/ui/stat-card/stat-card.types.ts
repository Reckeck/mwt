import type { SignTone } from '@/shared/lib/format'

export interface StatCardProps {
  /** What the figure is (`Total`, "Realized P&L"); the `label` slot swaps in a label with an explanation. */
  label: string
  /** The figure as text; or put `<Money>`/`<PnlText>` in the default slot instead. */
  value?: string
  /** One muted line under the figure. */
  hint?: string
  /** Colours a plain-text `value` by direction. */
  tone?: SignTone
  /** Change against a base (`4.2`, `-1`): its sign picks the mark and colour, see `Delta`. */
  delta?: number | string
  /** The change as formatted text after the sign (`4.2%`). */
  deltaText?: string
  /** A rise is bad (expenses): flips which direction is green. */
  deltaInverse?: boolean
  /** Shows a placeholder instead of a figure that would read as zero. */
  loading?: boolean
}
