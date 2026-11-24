/** Which `--chart-*` token paints the swatch. */
export type ChartLegendTone = 'actual' | 'reference' | 'income' | 'over' | 'bank' | 'crypto' | 'property' | 'accent'

export interface ChartLegendItem {
  label: string
  tone: ChartLegendTone
  /** A dashed swatch: the series is drawn as a dashed reference line. */
  dashed?: boolean
}

export interface ChartLegendProps {
  /** At least two: a legend of one series tells nothing and renders nothing. */
  items: ChartLegendItem[]
}
