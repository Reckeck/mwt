import type { Component } from 'vue'
import type { DrawingKind } from 'lightweight-charts-drawing'
import { ArrowUpRight, Brush, Minus, Ruler, Square, TrendingUp, Type } from 'lucide-vue-next'

/** Candle timeframes in minutes, as Binance klines accept them. */
export const CHART_TIMEFRAMES = [1, 3, 5, 15, 30, 60, 240, 1440] as const

export type IndicatorKey = 'sma' | 'ema' | 'rsi'

export const INDICATORS: { key: IndicatorKey; label: string }[] = [
  { key: 'sma', label: 'SMA 20' },
  { key: 'ema', label: 'EMA 50' },
  { key: 'rsi', label: 'RSI 14' },
]

export const DRAWING_TOOLS: { kind: DrawingKind; icon: Component; key: string }[] = [
  { kind: 'trend-line', icon: TrendingUp, key: 'trading.tool.trendLine' },
  { kind: 'horizontal-line', icon: Minus, key: 'trading.tool.horizontalLine' },
  { kind: 'fib-retracement', icon: Ruler, key: 'trading.tool.fib' },
  { kind: 'rectangle', icon: Square, key: 'trading.tool.rectangle' },
  { kind: 'brush', icon: Brush, key: 'trading.tool.brush' },
  { kind: 'text', icon: Type, key: 'trading.tool.text' },
  { kind: 'arrow', icon: ArrowUpRight, key: 'trading.tool.arrow' },
]
