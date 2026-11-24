import type { ChartLegendTone } from './chart-legend.types'

/** Static class names so Tailwind sees them. */
export const LEGEND_SWATCH_CLASS: Record<ChartLegendTone, string> = {
  actual: 'bg-chart-actual border-chart-actual',
  reference: 'bg-chart-reference border-chart-reference',
  income: 'bg-chart-income border-chart-income',
  over: 'bg-chart-over border-chart-over',
  bank: 'bg-chart-bank border-chart-bank',
  crypto: 'bg-chart-crypto border-chart-crypto',
  property: 'bg-chart-property border-chart-property',
  accent: 'bg-chart-accent border-chart-accent',
}
