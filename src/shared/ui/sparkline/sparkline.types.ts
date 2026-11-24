/** Which `--chart-*` token draws the line. */
export type SparklineTone = 'actual' | 'income' | 'over' | 'reference'

export interface SparklineProps {
  /** Points left to right; amounts come as decimal strings. */
  values: Array<number | string>
  /** Shade the area under the line. */
  area?: boolean
  /** Chart token of the line; a `text-*` class on the element overrides it. */
  tone?: SparklineTone
  /** Accessible summary; without it the line is decoration. */
  label?: string
}
