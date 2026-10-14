import { type ISeriesApi, LineSeries, LineStyle } from 'lightweight-charts'

import { chartPalette, withAlpha } from '../lib/chart-color'
import { ema, rsi, sma } from '../lib/indicators'
import type { IndicatorKey } from '../trading-chart.constants'
import type { ChartHandles } from './use-chart-core'

const PERIOD: Record<IndicatorKey, number> = { sma: 20, ema: 50, rsi: 14 }
const COMPUTE: Record<IndicatorKey, (values: number[], period: number) => (number | null)[]> = { sma, ema, rsi }

const overlayLine = (color: string) => ({
  color,
  lineWidth: 1 as const,
  priceLineVisible: false,
  lastValueVisible: false,
  crosshairMarkerVisible: false,
})

/** Recompute every shown indicator from the bars the chart holds. */
export function syncIndicators(handles: ChartHandles) {
  if (!handles.bars.length) return
  const closes = handles.bars.map((bar) => bar.close)
  for (const [key, line] of Object.entries(handles.lines) as [IndicatorKey, ISeriesApi<'Line'>][]) {
    const values = COMPUTE[key](closes, PERIOD[key])
    line.setData(
      handles.bars.map((bar, index) =>
        values[index] === null || values[index] === undefined
          ? { time: bar.time }
          : { time: bar.time, value: values[index] as number },
      ),
    )
  }
}

/**
 * Add the series of the wanted indicators and drop the rest. SMA and EMA sit
 * over the candles; RSI gets its own pane with the 70/30 levels.
 */
export function setIndicatorSeries(handles: ChartHandles, wanted: IndicatorKey[]) {
  const chart = handles.chart
  if (!chart || !handles.candles) return
  const colors = chartPalette()
  for (const key of ['sma', 'ema'] as const) {
    const line = handles.lines[key]
    if (wanted.includes(key) && !line) handles.lines[key] = chart.addSeries(LineSeries, overlayLine(colors[key]))
    else if (!wanted.includes(key) && line) {
      chart.removeSeries(line)
      delete handles.lines[key]
    }
  }
  if (wanted.includes('rsi') && !handles.lines.rsi) {
    const pane = chart.addPane()
    const line = pane.addSeries(LineSeries, {
      ...overlayLine(colors.rsi),
      autoscaleInfoProvider: () => ({ priceRange: { minValue: 0, maxValue: 100 } }),
    })
    for (const level of [70, 30]) {
      line.createPriceLine({
        price: level,
        color: withAlpha(colors.text, 0.6),
        lineWidth: 1,
        lineStyle: LineStyle.Dashed,
        axisLabelVisible: false,
        title: '',
      })
    }
    pane.setStretchFactor(0.3)
    handles.lines.rsi = line
  } else if (!wanted.includes('rsi') && handles.lines.rsi) {
    // Remove the whole pane: dropping only the series would leave an empty strip.
    chart.removePane(handles.lines.rsi.getPane().paneIndex())
    delete handles.lines.rsi
  }
  syncIndicators(handles)
}
