export type ProgressTone = 'primary' | 'destructive' | 'warning'
export type ProgressSize = 'sm' | 'md'

export interface ProgressBarProps {
  /** Percent, 0–100; clamped. */
  value: number
  tone?: ProgressTone
  /** `sm` 6px in dense widgets, `md` 8px. */
  size?: ProgressSize
  /** Percent, 0–100: a tick across the track where a limit sits (Budget row); clamped, none when omitted. */
  marker?: number
  /** What it measures, for a screen reader; without it the bar is decorative (the number is beside it). */
  label?: string
}
