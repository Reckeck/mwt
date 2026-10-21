export interface SparklineProps {
  /** Points left to right; amounts come as decimal strings. */
  values: Array<number | string>
  /** Shade the area under the line. */
  area?: boolean
  /** Accessible summary; without it the line is decoration. */
  label?: string
}
