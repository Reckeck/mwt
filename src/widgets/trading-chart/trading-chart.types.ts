export interface TradingChartProps {
  /** Base asset; quoted against USDT on Binance. */
  symbol: string
  /** How many candles to load. */
  limit?: number
}
