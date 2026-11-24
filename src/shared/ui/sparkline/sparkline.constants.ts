import type { SparklineTone } from './sparkline.types'

/** The line is drawn in `currentColor`, so the token is a text colour on the svg. */
export const SPARKLINE_TONE_CLASS: Record<SparklineTone, string> = {
  actual: 'text-chart-actual',
  income: 'text-chart-income',
  over: 'text-chart-over',
  reference: 'text-chart-reference',
}
