import { type Ref, computed, markRaw, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import {
  type BarData,
  CandlestickSeries,
  ColorType,
  CrosshairMode,
  HistogramSeries,
  type IChartApi,
  type ISeriesApi,
  type Time,
  type TickMarkType,
  type UTCTimestamp,
  createChart,
} from 'lightweight-charts'

import { type AppError, call, parseError } from '@/shared/api/ipc'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'

import { formatBarTime, formatTick, priceFormatFor } from '../lib/bar-format'
import { chartPalette, withAlpha } from '../lib/chart-color'
import type { IndicatorKey } from '../trading-chart.constants'
import { setIndicatorSeries as setIndicatorLines, syncIndicators as syncIndicatorLines } from './indicator-series'

export type ChartBar = BarData<UTCTimestamp> & { volume: number }

/**
 * Everything the chart library owns. Kept out of Vue's reactivity (`markRaw`):
 * proxies break lightweight-charts, and the chart renders on its own.
 */
export interface ChartHandles {
  chart: IChartApi | null
  candles: ISeriesApi<'Candlestick'> | null
  volume: ISeriesApi<'Histogram'> | null
  lines: Partial<Record<IndicatorKey, ISeriesApi<'Line'>>>
  bars: ChartBar[]
  /** Volume by bar time: the candlestick series does not carry it back on hover. */
  volumeAt: Map<number, number>
  precision: number
  alive: boolean
}

export interface ChartLegend {
  time: string
  open: number
  high: number
  low: number
  close: number
  volume: number
}

/**
 * The chart itself: created on mount in `el`, themed from the app's tokens,
 * fed with candles from the backend, its indicators and the legend of the
 * hovered bar. `onReady` runs once the chart exists (drawings, the stream).
 */
export function useChartCore(options: {
  el: Ref<HTMLDivElement | null>
  symbol: () => string
  limit: () => number
  tf: Ref<number>
  indicators: Ref<IndicatorKey[]>
  onReady: (handles: ChartHandles) => void
}) {
  const { resolved: theme } = useTheme()
  const handles: ChartHandles = markRaw({
    chart: null,
    candles: null,
    volume: null,
    lines: {},
    bars: [],
    volumeAt: new Map(),
    precision: 2,
    alive: true,
  })

  const loading = ref(false)
  const error = ref<AppError | null>(null)
  /** Whether the chart currently holds any bars; overlays only cover an empty chart. */
  const hasBars = ref(false)
  const legend = ref<ChartLegend | null>(null)
  const change = computed(() =>
    legend.value?.open ? ((legend.value.close - legend.value.open) / legend.value.open) * 100 : 0,
  )
  const fmt = (value: number): string => value.toFixed(handles.precision)
  const barTime = (time: Time) => formatBarTime(time, options.tf.value, activeLocale.value)

  function applyTheme() {
    if (!handles.chart) return
    const colors = chartPalette()
    handles.chart.applyOptions({
      layout: {
        background: { type: ColorType.Solid, color: colors.background },
        textColor: colors.text,
        attributionLogo: false,
        panes: { separatorColor: colors.border, separatorHoverColor: withAlpha(colors.text, 0.2) },
      },
      grid: {
        vertLines: { color: withAlpha(colors.border, 0.6) },
        horzLines: { color: withAlpha(colors.border, 0.6) },
      },
      crosshair: {
        vertLine: { color: withAlpha(colors.text, 0.5), labelBackgroundColor: colors.primary },
        horzLine: { color: withAlpha(colors.text, 0.5), labelBackgroundColor: colors.primary },
      },
      rightPriceScale: { borderColor: colors.border },
      timeScale: { borderColor: colors.border },
    })
    handles.candles?.applyOptions({
      upColor: colors.up,
      downColor: colors.down,
      borderUpColor: colors.up,
      borderDownColor: colors.down,
      wickUpColor: colors.up,
      wickDownColor: colors.down,
    })
    for (const [key, line] of Object.entries(handles.lines)) line?.applyOptions({ color: colors[key as IndicatorKey] })
    if (handles.bars.length) handles.volume?.setData(handles.bars.map(volumeBar))
  }

  function showBar(bar: BarData<UTCTimestamp>) {
    legend.value = {
      time: barTime(bar.time),
      open: bar.open,
      high: bar.high,
      low: bar.low,
      close: bar.close,
      volume: handles.volumeAt.get(Number(bar.time)) ?? 0,
    }
  }

  const syncIndicators = () => syncIndicatorLines(handles)
  const setIndicatorSeries = () => setIndicatorLines(handles, options.indicators.value)

  /* --- data --- */
  function volumeBar(bar: ChartBar) {
    const colors = chartPalette()
    return { time: bar.time, value: bar.volume, color: withAlpha(bar.close >= bar.open ? colors.up : colors.down, 0.4) }
  }

  function setData(fit: boolean) {
    const { candles, volume, bars } = handles
    if (!candles || !volume) return
    hasBars.value = bars.length > 0
    handles.volumeAt.clear()
    for (const bar of bars) handles.volumeAt.set(Number(bar.time), bar.volume)
    if (!bars.length) {
      candles.setData([])
      volume.setData([])
      legend.value = null
      return
    }
    const last = bars[bars.length - 1]
    handles.precision = priceFormatFor(last.close).precision
    candles.applyOptions({ priceFormat: priceFormatFor(last.close) })
    candles.setData(bars)
    volume.setData(bars.map(volumeBar))
    syncIndicators()
    showBar(last)
    if (fit) handles.chart?.timeScale().fitContent()
  }

  async function load() {
    loading.value = true
    error.value = null
    try {
      const out = await call<GetCandlesOutput>('get_candles', {
        input: { symbol: options.symbol(), interval_min: options.tf.value, limit: options.limit() },
      })
      if (!handles.alive) return
      handles.bars = out.candles.map((candle) => ({
        time: Math.floor(Date.parse(candle.ts) / 1000) as UTCTimestamp,
        open: Number(candle.open),
        high: Number(candle.high),
        low: Number(candle.low),
        close: Number(candle.close),
        volume: Number(candle.volume),
      }))
      setData(true)
    } catch (err) {
      if (!handles.alive) return
      handles.bars = []
      setData(false)
      error.value = parseError(err)
    } finally {
      if (handles.alive) loading.value = false
    }
  }

  /** A live bar from the stream: a new one is appended, the current one replaced. */
  function pushBar(bar: ChartBar) {
    const last = handles.bars[handles.bars.length - 1]
    if (!last || bar.time > last.time) handles.bars.push(bar)
    else if (bar.time === last.time) handles.bars[handles.bars.length - 1] = bar
    else return
    handles.volumeAt.set(Number(bar.time), bar.volume)
    handles.candles?.update(bar)
    handles.volume?.update(volumeBar(bar))
    syncIndicators()
    showBar(bar)
  }

  onMounted(() => {
    const el = options.el.value
    if (!el) return
    const chart = createChart(el, {
      autoSize: true,
      crosshair: { mode: CrosshairMode.Normal },
      // Leave the bottom quarter to the volume overlay.
      rightPriceScale: { scaleMargins: { top: 0.1, bottom: 0.25 } },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
        rightOffset: 4,
        tickMarkFormatter: (time: Time, type: TickMarkType) => formatTick(time, type, activeLocale.value),
      },
      localization: { locale: activeLocale.value, timeFormatter: barTime },
    })
    handles.chart = chart
    handles.candles = chart.addSeries(CandlestickSeries, {})
    // Volume sits under the candles on its own hidden scale instead of a
    // separate pane, which read as an empty strip at most zoom levels.
    handles.volume = chart.addSeries(HistogramSeries, {
      priceFormat: { type: 'volume' },
      priceScaleId: 'volume',
      priceLineVisible: false,
      lastValueVisible: false,
    })
    chart.priceScale('volume').applyOptions({ scaleMargins: { top: 0.8, bottom: 0 }, visible: false })
    chart.subscribeCrosshairMove((param) => {
      const bar = (handles.candles ? param.seriesData.get(handles.candles) : undefined) as
        | BarData<UTCTimestamp>
        | undefined
      if (bar && param.time !== undefined) showBar(bar)
      else if (handles.bars.length) showBar(handles.bars[handles.bars.length - 1])
    })
    applyTheme()
    setIndicatorSeries()
    options.onReady(handles)
    void load()
  })

  onBeforeUnmount(() => {
    handles.alive = false
    handles.chart?.remove()
    handles.chart = null
    handles.candles = null
    handles.volume = null
    handles.lines = {}
  })

  watch(theme, applyTheme)
  watch(activeLocale, (locale) => {
    handles.chart?.applyOptions({ localization: { locale } })
    if (handles.bars.length) showBar(handles.bars[handles.bars.length - 1])
  })
  watch(options.indicators, setIndicatorSeries)

  return { handles, loading, error, hasBars, legend, change, fmt, load, pushBar }
}
