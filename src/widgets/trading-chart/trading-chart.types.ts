import type { IndicatorKey } from './trading-chart.constants'

export interface TradingChartProps {
  /** Base asset; quoted against USDT on Binance. */
  symbol: string
  /** How many candles to load. */
  limit?: number
  /** Saved drawing exports by upper-case symbol; the chart reports changes through `drawings`. */
  savedDrawings?: Record<string, string>
  /** Initial timeframe label (`1h`, `15m`, `1d`). */
  timeframe?: string
  /** Initially shown indicators. */
  indicators?: IndicatorKey[]
}
