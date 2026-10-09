<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  CandlestickSeries,
  ColorType,
  CrosshairMode,
  HistogramSeries,
  LineSeries,
  LineStyle,
  TickMarkType,
  createChart,
  type BarData,
  type IChartApi,
  type ISeriesApi,
  type Time,
  type UTCTimestamp,
} from 'lightweight-charts'
import { DrawingManager, type DrawingKind } from 'lightweight-charts-drawing'
import { Magnet, RefreshCw, Trash2, Undo2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Separator } from '@/shared/ui/separator'
import { Toggle } from '@/shared/ui/toggle'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'
import { call, parseError, type AppError } from '@/shared/api/ipc'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'
import { cn } from '@/shared/lib/utils'

import { CHART_TIMEFRAMES, DRAWING_TOOLS, INDICATORS, type IndicatorKey } from './trading-chart.constants'
import type { TradingChartProps } from './trading-chart.types'

const props = withDefaults(defineProps<TradingChartProps>(), { limit: 500 })

const { t } = useI18n()
const { resolved: theme } = useTheme()

const tf = ref(60)
const chartEl = ref<HTMLDivElement | null>(null)
const loading = ref(false)
const error = ref<AppError | null>(null)
/** Whether the chart currently holds any bars; overlays only cover an empty chart. */
const hasBars = ref(false)
/** Binance stream status shown as a Live/Connecting/Offline chip. */
const stream = ref<'connecting' | 'live' | 'down'>('down')
const legend = ref<{ time: string; open: number; high: number; low: number; close: number; volume: number } | null>(
  null,
)
const indicators = ref<IndicatorKey[]>(['sma'])
const activeTool = ref<DrawingKind | null>(null)
const magnet = ref(true)
const drawingCount = ref(0)
const undoStack = ref<string[]>([])

const tfLabel = (min: number): string => {
  if (min === 60) return '1h'
  if (min === 240) return '4h'
  if (min === 1440) return '1d'
  return `${min}m`
}

// --- chart core -------------------------------------------------------------

let chart: IChartApi | null = null
let candles: ISeriesApi<'Candlestick'> | null = null
let volume: ISeriesApi<'Histogram'> | null = null
let smaLine: ISeriesApi<'Line'> | null = null
let emaLine: ISeriesApi<'Line'> | null = null
let rsiLine: ISeriesApi<'Line'> | null = null
let drawings: DrawingManager | null = null

/** OHLCV in chart form; kept outside reactivity, the chart owns rendering. */
let bars: (BarData<UTCTimestamp> & { volume: number })[] = []
/** Volume by bar time: the candlestick series does not carry it back on hover. */
const volumeAt = new Map<number, number>()
let precision = 2

let mounted = true
let ws: WebSocket | null = null
let wsRetry: ReturnType<typeof setTimeout> | null = null
let wsAttempts = 0

/** Guard so importJSON/clear do not feed back into persistence. */
let applying = false

const drawKey = computed(() => `tradeapp.drawings.${props.symbol}`)

const UP = '#26a69a'
const DOWN = '#ef5350'

const probe = (() => {
  const canvas = document.createElement('canvas')
  canvas.width = 1
  canvas.height = 1
  return canvas.getContext('2d', { willReadFrequently: true })
})()

/**
 * Resolve a CSS variable to `rgba(…)`.
 *
 * The theme is written in oklch, and Chromium hands oklch/lab strings straight
 * back from both getComputedStyle and canvas fillStyle — which the chart cannot
 * parse ("Failed to parse color: lab(…)"). Painting one pixel and reading it
 * back yields plain sRGB whatever the input syntax.
 */
function cssColor(name: string, fallback: string): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  if (!probe || !raw) return fallback
  probe.clearRect(0, 0, 1, 1)
  probe.fillStyle = fallback
  probe.fillStyle = raw
  probe.fillRect(0, 0, 1, 1)
  const [red, green, blue, opacity] = probe.getImageData(0, 0, 1, 1).data
  return `rgba(${red}, ${green}, ${blue}, ${(opacity / 255).toFixed(3)})`
}

/** Scale a resolved colour's opacity; keeps the alpha it already had (borders are translucent). */
function withAlpha(color: string, factor: number): string {
  const match = color.match(/rgba?\(([^)]+)\)/)
  if (!match) return color
  const [red, green, blue, opacity = '1'] = match[1].split(/[,\s/]+/).filter(Boolean)
  return `rgba(${red}, ${green}, ${blue}, ${(Number(opacity) * factor).toFixed(3)})`
}

function applyTheme() {
  if (!chart) return
  const dark = theme.value === 'dark'
  const background = cssColor('--card', dark ? '#18181b' : '#ffffff')
  const text = cssColor('--muted-foreground', '#787b86')
  const border = cssColor('--border', dark ? '#2a2e39' : '#e4e4e7')
  const primary = cssColor('--primary', '#2962ff')
  chart.applyOptions({
    layout: { background: { type: ColorType.Solid, color: background }, textColor: text, attributionLogo: false },
    grid: {
      vertLines: { color: withAlpha(border, 0.6) },
      horzLines: { color: withAlpha(border, 0.6) },
    },
    crosshair: {
      vertLine: { color: withAlpha(text, 0.5), labelBackgroundColor: primary },
      horzLine: { color: withAlpha(text, 0.5), labelBackgroundColor: primary },
    },
    rightPriceScale: { borderColor: border },
    timeScale: { borderColor: border },
  })
}

// --- time: one local formatter for the axis, the crosshair and the legend ---

/** Bars are UTC seconds; every label is rendered in the user's own time zone. */
const barDate = (time: Time): Date => new Date(Number(time) * 1000)

function formatBarTime(time: Time): string {
  const daily = tf.value >= 1440
  return barDate(time).toLocaleString(activeLocale.value, {
    day: 'numeric',
    month: 'short',
    year: daily ? 'numeric' : undefined,
    hour: daily ? undefined : '2-digit',
    minute: daily ? undefined : '2-digit',
  })
}

function formatTick(time: Time, type: TickMarkType): string {
  const date = barDate(time)
  const locale = activeLocale.value
  switch (type) {
    case TickMarkType.Year:
      return date.toLocaleDateString(locale, { year: 'numeric' })
    case TickMarkType.Month:
      return date.toLocaleDateString(locale, { month: 'short' })
    case TickMarkType.DayOfMonth:
      return date.toLocaleDateString(locale, { day: 'numeric', month: 'short' })
    default:
      return date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })
  }
}

function priceFormatFor(price: number) {
  if (price >= 1000) return { type: 'price' as const, precision: 2, minMove: 0.01 }
  if (price >= 1) return { type: 'price' as const, precision: 4, minMove: 0.0001 }
  return { type: 'price' as const, precision: 8, minMove: 1e-8 }
}

const fmt = (value: number): string => value.toFixed(precision)

const change = computed(() => {
  if (!legend.value?.open) return 0
  return ((legend.value.close - legend.value.open) / legend.value.open) * 100
})

// --- indicators -------------------------------------------------------------

function smaArr(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = []
  let sum = 0
  for (let index = 0; index < values.length; index++) {
    sum += values[index]
    if (index >= period) sum -= values[index - period]
    out.push(index >= period - 1 ? sum / period : null)
  }
  return out
}

function emaArr(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = []
  const multiplier = 2 / (period + 1)
  let prev: number | null = null
  for (let index = 0; index < values.length; index++) {
    if (index < period - 1) {
      out.push(null)
      continue
    }
    if (prev === null) {
      let seed = 0
      for (let from = index - period + 1; from <= index; from++) seed += values[from]
      prev = seed / period
      out.push(prev)
      continue
    }
    prev = values[index] * multiplier + prev * (1 - multiplier)
    out.push(prev)
  }
  return out
}

function rsiArr(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = new Array(values.length).fill(null)
  if (values.length <= period) return out
  let gain = 0
  let loss = 0
  for (let index = 1; index <= period; index++) {
    const delta = values[index] - values[index - 1]
    if (delta >= 0) gain += delta
    else loss -= delta
  }
  let avgGain = gain / period
  let avgLoss = loss / period
  const toRsi = () => (avgLoss === 0 ? 100 : 100 - 100 / (1 + avgGain / avgLoss))
  out[period] = toRsi()
  for (let index = period + 1; index < values.length; index++) {
    const delta = values[index] - values[index - 1]
    avgGain = (avgGain * (period - 1) + Math.max(delta, 0)) / period
    avgLoss = (avgLoss * (period - 1) + Math.max(-delta, 0)) / period
    out[index] = toRsi()
  }
  return out
}

function lineData(values: (number | null)[]): { time: UTCTimestamp; value?: number }[] {
  return bars.map((bar, index) =>
    values[index] === null || values[index] === undefined
      ? { time: bar.time }
      : { time: bar.time, value: values[index] as number },
  )
}

function syncIndicators() {
  if (!bars.length) return
  const closes = bars.map((bar) => bar.close)
  if (smaLine) smaLine.setData(lineData(smaArr(closes, 20)))
  if (emaLine) emaLine.setData(lineData(emaArr(closes, 50)))
  if (rsiLine) rsiLine.setData(lineData(rsiArr(closes, 14)))
}

const overlayLine = (color: string) => ({
  color,
  lineWidth: 1 as const,
  priceLineVisible: false,
  lastValueVisible: false,
  crosshairMarkerVisible: false,
})

function setIndicatorSeries() {
  if (!chart || !candles) return
  const wants = (key: IndicatorKey) => indicators.value.includes(key)

  if (wants('sma') && !smaLine) smaLine = chart.addSeries(LineSeries, overlayLine('#f5a623'))
  else if (!wants('sma') && smaLine) {
    chart.removeSeries(smaLine)
    smaLine = null
  }

  if (wants('ema') && !emaLine) emaLine = chart.addSeries(LineSeries, overlayLine('#2962ff'))
  else if (!wants('ema') && emaLine) {
    chart.removeSeries(emaLine)
    emaLine = null
  }

  if (wants('rsi') && !rsiLine) {
    const pane = chart.addPane()
    rsiLine = pane.addSeries(LineSeries, {
      ...overlayLine('#9b6dff'),
      autoscaleInfoProvider: () => ({ priceRange: { minValue: 0, maxValue: 100 } }),
    })
    for (const level of [70, 30]) {
      rsiLine.createPriceLine({
        price: level,
        color: 'rgba(120, 123, 134, 0.6)',
        lineWidth: 1,
        lineStyle: LineStyle.Dashed,
        axisLabelVisible: false,
        title: '',
      })
    }
    pane.setStretchFactor(0.3)
  } else if (!wants('rsi') && rsiLine) {
    // Remove the whole pane: dropping only the series would leave an empty strip.
    chart.removePane(rsiLine.getPane().paneIndex())
    rsiLine = null
  }
  syncIndicators()
}

// --- data -------------------------------------------------------------------

const volumeBar = (bar: { time: UTCTimestamp; open: number; close: number; volume: number }) => ({
  time: bar.time,
  value: bar.volume,
  color: withAlpha(bar.close >= bar.open ? 'rgb(38, 166, 154)' : 'rgb(239, 83, 80)', 0.4),
})

function setData(fit: boolean) {
  if (!candles || !volume) return
  hasBars.value = bars.length > 0
  volumeAt.clear()
  for (const bar of bars) volumeAt.set(Number(bar.time), bar.volume)
  if (!bars.length) {
    candles.setData([])
    volume.setData([])
    legend.value = null
    return
  }
  const last = bars[bars.length - 1]
  precision = priceFormatFor(last.close).precision
  candles.applyOptions({ priceFormat: priceFormatFor(last.close) })
  candles.setData(bars)
  volume.setData(bars.map(volumeBar))
  syncIndicators()
  showBar(last)
  if (fit) chart?.timeScale().fitContent()
}

async function load() {
  loading.value = true
  error.value = null
  try {
    const out = await call<GetCandlesOutput>('get_candles', {
      input: { symbol: props.symbol, interval_min: tf.value, limit: props.limit },
    })
    if (!mounted) return
    bars = out.candles.map((candle) => ({
      time: Math.floor(Date.parse(candle.ts) / 1000) as UTCTimestamp,
      open: Number(candle.open),
      high: Number(candle.high),
      low: Number(candle.low),
      close: Number(candle.close),
      volume: Number(candle.volume),
    }))
    setData(true)
  } catch (err) {
    if (!mounted) return
    bars = []
    setData(false)
    error.value = parseError(err)
  } finally {
    if (mounted) loading.value = false
  }
}

function showBar(bar: BarData<UTCTimestamp>) {
  legend.value = {
    time: formatBarTime(bar.time),
    open: bar.open,
    high: bar.high,
    low: bar.low,
    close: bar.close,
    volume: volumeAt.get(Number(bar.time)) ?? 0,
  }
}

// --- realtime stream --------------------------------------------------------

function disconnectStream() {
  if (wsRetry) {
    clearTimeout(wsRetry)
    wsRetry = null
  }
  if (ws) {
    ws.onclose = null
    ws.onerror = null
    ws.close()
    ws = null
  }
}

function scheduleReconnect() {
  if (!mounted) return
  stream.value = 'connecting'
  const delay = Math.min(1000 * 2 ** wsAttempts, 30_000)
  wsAttempts++
  wsRetry = setTimeout(connectStream, delay)
}

function connectStream() {
  disconnectStream()
  if (!mounted || !props.symbol) return
  const pair = `${props.symbol.toLowerCase()}usdt`
  const url = `wss://stream.binance.com:9443/ws/${pair}@kline_${tfLabel(tf.value)}`
  try {
    ws = new WebSocket(url)
  } catch {
    scheduleReconnect()
    return
  }
  ws.onopen = () => {
    wsAttempts = 0
    stream.value = 'live'
  }
  ws.onerror = () => ws?.close()
  ws.onclose = () => {
    if (ws) {
      ws = null
      stream.value = 'down'
      scheduleReconnect()
    }
  }
  ws.onmessage = (ev) => {
    try {
      const msg = JSON.parse(String(ev.data)) as {
        k?: { t: number; o: string; h: string; l: string; c: string; v: string }
      }
      const ticker = msg.k
      if (!ticker) return
      stream.value = 'live'
      const bar = {
        time: Math.floor(ticker.t / 1000) as UTCTimestamp,
        open: Number(ticker.o),
        high: Number(ticker.h),
        low: Number(ticker.l),
        close: Number(ticker.c),
        volume: Number(ticker.v),
      }
      const last = bars[bars.length - 1]
      if (!last || bar.time > last.time) bars.push(bar)
      else if (bar.time === last.time) bars[bars.length - 1] = bar
      else return
      volumeAt.set(Number(bar.time), bar.volume)
      candles?.update(bar)
      volume?.update(volumeBar(bar))
      syncIndicators()
      showBar(bar)
    } catch {
      // malformed frame — ignore
    }
  }
}

// --- drawings ---------------------------------------------------------------

function persist(key: string) {
  if (applying || !drawings) return
  localStorage.setItem(key, drawings.exportJSON())
}

function loadDrawings(key: string) {
  if (!drawings) return
  applying = true
  try {
    drawings.importJSON(localStorage.getItem(key) ?? '[]')
  } finally {
    applying = false
  }
  undoStack.value = []
  drawingCount.value = drawings.drawings().length
}

function initDrawings() {
  if (!chart || !candles) return
  drawings = new DrawingManager(chart, candles, { magnet: magnet.value ? 'weak' : 'off' })
  drawings.setInterval(tfLabel(tf.value))
  drawings.on('add', (drawing) => {
    undoStack.value.push(drawing.id)
    drawingCount.value = drawings?.drawings().length ?? 0
    persist(drawKey.value)
  })
  drawings.on('remove', (drawing) => {
    undoStack.value = undoStack.value.filter((id) => id !== drawing.id)
    drawingCount.value = drawings?.drawings().length ?? 0
    persist(drawKey.value)
  })
  drawings.on('change', () => {
    drawingCount.value = drawings?.drawings().length ?? 0
  })
  drawings.on('tool', () => {
    activeTool.value = drawings?.tool() ?? null
  })
  loadDrawings(drawKey.value)
}

/** A single-choice group emits '' when the active item is pressed again: that means "no tool". */
function pickTool(value: unknown) {
  const next = (typeof value === 'string' && value ? value : null) as DrawingKind | null
  drawings?.setTool(next)
  activeTool.value = next
}

function undoDrawing() {
  if (!drawings) return
  const id = undoStack.value.pop()
  if (id) drawings.remove(id)
}

function clearDrawings() {
  if (!drawings) return
  applying = true
  try {
    drawings.clear()
  } finally {
    applying = false
  }
  undoStack.value = []
  drawingCount.value = 0
  persist(drawKey.value)
}

/** The timeframe always has a value: pressing the active one again is ignored. */
function pickTimeframe(value: unknown) {
  if (typeof value === 'string' && value) tf.value = Number(value)
}

function pickIndicators(value: unknown) {
  indicators.value = Array.isArray(value) ? (value as IndicatorKey[]) : []
}

// --- lifecycle --------------------------------------------------------------

onMounted(() => {
  const el = chartEl.value
  if (!el) return
  chart = createChart(el, {
    autoSize: true,
    crosshair: { mode: CrosshairMode.Normal },
    // Leave the bottom quarter to the volume overlay.
    rightPriceScale: { scaleMargins: { top: 0.1, bottom: 0.25 } },
    timeScale: {
      timeVisible: true,
      secondsVisible: false,
      rightOffset: 4,
      tickMarkFormatter: (time: Time, type: TickMarkType) => formatTick(time, type),
    },
    localization: { locale: activeLocale.value, timeFormatter: formatBarTime },
  })
  candles = chart.addSeries(CandlestickSeries, {
    upColor: UP,
    downColor: DOWN,
    borderUpColor: UP,
    borderDownColor: DOWN,
    wickUpColor: UP,
    wickDownColor: DOWN,
  })
  // Volume sits under the candles on its own hidden scale instead of a
  // separate pane, which read as an empty strip at most zoom levels.
  volume = chart.addSeries(HistogramSeries, {
    priceFormat: { type: 'volume' },
    priceScaleId: 'volume',
    priceLineVisible: false,
    lastValueVisible: false,
  })
  chart.priceScale('volume').applyOptions({ scaleMargins: { top: 0.8, bottom: 0 }, visible: false })
  chart.subscribeCrosshairMove((param) => {
    const bar = (candles ? param.seriesData.get(candles) : undefined) as BarData<UTCTimestamp> | undefined
    if (bar && param.time !== undefined) showBar(bar)
    else if (bars.length) showBar(bars[bars.length - 1])
  })
  applyTheme()
  setIndicatorSeries()
  initDrawings()
  load()
  connectStream()
})

onBeforeUnmount(() => {
  mounted = false
  disconnectStream()
  drawings?.destroy()
  drawings = null
  chart?.remove()
  chart = null
  candles = null
  volume = null
  smaLine = null
  emaLine = null
  rsiLine = null
})

watch(theme, applyTheme)

watch(activeLocale, (locale) => {
  chart?.applyOptions({ localization: { locale } })
  if (bars.length) showBar(bars[bars.length - 1])
})

watch(
  () => props.symbol,
  (_next, prev) => {
    if (prev) localStorage.setItem(`tradeapp.drawings.${prev}`, drawings?.exportJSON() ?? '[]')
    loadDrawings(drawKey.value)
    load()
    wsAttempts = 0
    connectStream()
  },
)

watch(tf, () => {
  drawings?.setInterval(tfLabel(tf.value))
  load()
  wsAttempts = 0
  connectStream()
})

watch(indicators, setIndicatorSeries)

watch(magnet, (on) => drawings?.setMagnet(on ? 'weak' : 'off'))
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <ToggleGroup
        type="single"
        variant="outline"
        size="sm"
        :model-value="String(tf)"
        :aria-label="t('trading.timeframe')"
        @update:model-value="pickTimeframe"
      >
        <ToggleGroupItem v-for="frame in CHART_TIMEFRAMES" :key="frame" :value="String(frame)" class="tabular-nums">
          {{ tfLabel(frame) }}
        </ToggleGroupItem>
      </ToggleGroup>

      <ToggleGroup
        type="multiple"
        variant="outline"
        size="sm"
        :model-value="indicators"
        :aria-label="t('trading.indicators')"
        @update:model-value="pickIndicators"
      >
        <ToggleGroupItem v-for="ind in INDICATORS" :key="ind.key" :value="ind.key">
          {{ ind.label }}
        </ToggleGroupItem>
      </ToggleGroup>

      <div class="flex items-center gap-1">
        <ToggleGroup
          type="single"
          size="sm"
          :model-value="activeTool ?? ''"
          :aria-label="t('trading.drawing')"
          @update:model-value="pickTool"
        >
          <Tooltip v-for="tool in DRAWING_TOOLS" :key="tool.kind">
            <TooltipTrigger as-child>
              <ToggleGroupItem :value="tool.kind" :aria-label="t(tool.key)">
                <component :is="tool.icon" />
              </ToggleGroupItem>
            </TooltipTrigger>
            <TooltipContent>{{ t(tool.key) }}</TooltipContent>
          </Tooltip>
        </ToggleGroup>

        <Separator orientation="vertical" class="mx-1 data-[orientation=vertical]:h-5" />

        <Tooltip>
          <TooltipTrigger as-child>
            <Toggle v-model="magnet" size="sm" :aria-label="t('trading.magnet')">
              <Magnet />
            </Toggle>
          </TooltipTrigger>
          <TooltipContent>{{ t('trading.magnet') }}</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="size-8"
              :aria-label="t('trading.undo')"
              :disabled="!undoStack.length"
              @click="undoDrawing"
            >
              <Undo2 class="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{{ t('trading.undo') }}</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              size="icon"
              class="size-8"
              :aria-label="t('trading.clearDrawings')"
              :disabled="!drawingCount"
              @click="clearDrawings"
            >
              <Trash2 class="size-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>{{ t('trading.clearDrawings') }}</TooltipContent>
        </Tooltip>
      </div>

      <div class="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
        <RefreshCw v-if="loading && hasBars" class="size-3.5 animate-spin" />
        <span
          class="size-2 rounded-full"
          :class="
            cn(
              stream === 'live' && 'bg-emerald-500',
              stream === 'connecting' && 'bg-amber-500',
              stream === 'down' && 'bg-muted-foreground',
            )
          "
        />
        {{ stream === 'live' ? t('trading.live') : stream === 'connecting' ? t('trading.connecting') : t('trading.down') }}
      </div>
    </div>

    <div class="relative min-h-0 flex-1 overflow-hidden rounded-lg border border-border bg-card">
      <div ref="chartEl" class="absolute inset-0" />

      <!-- OHLC of the hovered bar (or the last one); same clock as the axis. -->
      <div
        v-if="legend"
        class="pointer-events-none absolute left-2 top-2 z-10 flex max-w-[calc(100%-6rem)] flex-wrap items-center gap-x-3 gap-y-0.5 rounded-md bg-card/80 px-2 py-1 text-xs tabular-nums text-muted-foreground backdrop-blur-sm"
      >
        <span class="font-semibold text-foreground">{{ symbol }} · {{ tfLabel(tf) }}</span>
        <span>{{ legend.time }}</span>
        <span>{{ t('trading.o') }} <span class="text-foreground">{{ fmt(legend.open) }}</span></span>
        <span>{{ t('trading.h') }} <span class="text-foreground">{{ fmt(legend.high) }}</span></span>
        <span>{{ t('trading.l') }} <span class="text-foreground">{{ fmt(legend.low) }}</span></span>
        <span>{{ t('trading.c') }} <span class="text-foreground">{{ fmt(legend.close) }}</span></span>
        <span :class="change >= 0 ? 'text-emerald-500' : 'text-red-500'">
          {{ change >= 0 ? '+' : '' }}{{ change.toFixed(2) }}%
        </span>
        <span>{{ t('trading.vol') }} <span class="text-foreground">{{ Math.round(legend.volume).toLocaleString(activeLocale) }}</span></span>
      </div>

      <div
        v-if="!hasBars && (loading || error)"
        class="absolute inset-0 z-20 flex flex-col items-center justify-center gap-3 bg-card px-6 text-center text-sm"
      >
        <template v-if="loading">
          <RefreshCw class="size-5 animate-spin text-muted-foreground" />
          <span class="text-muted-foreground">{{ t('trading.loading') }}</span>
        </template>
        <template v-else-if="error">
          <span class="text-destructive">{{ error.message }}</span>
          <Button variant="outline" size="sm" @click="load">
            <RefreshCw class="size-4" />
            {{ t('actions.refresh') }}
          </Button>
        </template>
      </div>
      <div
        v-else-if="!hasBars && !loading"
        class="absolute inset-0 z-20 flex items-center justify-center bg-card px-6 text-center text-sm text-muted-foreground"
      >
        {{ t('trading.empty') }}
      </div>
    </div>
  </div>
</template>
