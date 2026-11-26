import type { ChartColors } from '@/shared/lib/chart-theme'
import { axisMoney } from '@/shared/lib/chart-theme'
import type { ChartOption } from '@/shared/lib/echarts'
import {
  kitActualSeries,
  kitCategoryAxis,
  kitGrid,
  kitTooltip,
  kitValueAxis,
  type MarkerPoint,
} from '@/shared/lib/echarts-kit'

/** The category label of a candle: the hour for a short step, the day otherwise. */
export function candleLabel(ts: string, intervalMin: number): string {
  return intervalMin < 1440 ? ts.slice(5, 16).replace('T', ' ') : ts.slice(0, 10)
}

/** The highest and the lowest close of the span, as the two markers a reader looks for. */
export function candleExtremes(labels: string[], closes: number[]): MarkerPoint[] {
  if (closes.length === 0) return []
  const high = closes.indexOf(Math.max(...closes))
  const low = closes.indexOf(Math.min(...closes))
  const points = [{ at: labels[high], value: closes[high] }]
  if (low !== high) points.push({ at: labels[low], value: closes[low] })
  return points
}

/** Closing price as one line in `--chart-actual` under a `--chart-wash` area, the extremes ringed; all axes from the kit. */
export function candleOption(
  colors: ChartColors,
  input: { candles: CandlePoint[]; intervalMin: number; name: string; currency: string },
): ChartOption {
  const labels = input.candles.map((candle) => candleLabel(candle.ts, input.intervalMin))
  const closes = input.candles.map((candle) => Number(candle.close))
  const series = {
    ...kitActualSeries(colors, { name: input.name, data: closes }, candleExtremes(labels, closes)),
    areaStyle: { color: colors.wash },
  }
  return {
    grid: kitGrid(),
    xAxis: kitCategoryAxis(colors, labels),
    yAxis: kitValueAxis(colors),
    tooltip: { ...kitTooltip(colors), valueFormatter: (value: unknown) => axisMoney(Number(value), input.currency) },
    series: [series],
  } as ChartOption
}
