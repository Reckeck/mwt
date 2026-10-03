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
  createChart,
  createTextWatermark,
  type BarData,
  type IChartApi,
  type ISeriesApi,
  type ITextWatermarkPluginApi,
  type Time,
  type UTCTimestamp,
} from 'lightweight-charts'
import { DrawingManager, type DrawingKind } from 'lightweight-charts-drawing'
import {
  ArrowUpRight,
  Brush,
  Magnet,
  Minus,
  Ruler,
  Square,
  Trash2,
  TrendingUp,
  Type,
  Undo2,
} from 'lucide-vue-next'

import Button from '@/components/ui/button.vue'
import Tooltip from '@/components/ui/tooltip.vue'
import { call, parseError, type AppError } from '@/lib/ipc'
import { useTheme } from '@/theme'

const props = withDefaults(
  defineProps<{ symbol: string; limit?: number }>(),
  { limit: 500 },
)

const { t } = useI18n()
const { resolved: theme } = useTheme()

const TIMEFRAMES = [1, 3, 5, 15, 30, 60, 240, 1440]
const tf = ref(60)

const chartEl = ref<HTMLDivElement | null>(null)
const loading = ref(false)
const error = ref<AppError | null>(null)
const empty = ref(false)
/** Binance stream status shown as a Live/Connecting/Offline chip. */
const stream = ref<'connecting' | 'live' | 'down'>('down')
const legend = ref<{ time: string; o: number; h: number; l: number; c: number; v: number } | null>(null)
const indicators = ref({ sma: true, ema: false, rsi: false })
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

const tools: { kind: DrawingKind; icon: typeof TrendingUp; key: string }[] = [
  { kind: 'trend-line', icon: TrendingUp, key: 'trading.tool.trendLine' },
  { kind: 'horizontal-line', icon: Minus, key: 'trading.tool.horizontalLine' },
  { kind: 'fib-retracement', icon: Ruler, key: 'trading.tool.fib' },
  { kind: 'rectangle', icon: Square, key: 'trading.tool.rectangle' },
  { kind: 'brush', icon: Brush, key: 'trading.tool.brush' },
  { kind: 'text', icon: Type, key: 'trading.tool.text' },
  { kind: 'arrow', icon: ArrowUpRight, key: 'trading.tool.arrow' },
]

type IndicatorKey = 'sma' | 'ema' | 'rsi'
const indicatorToggles: { key: IndicatorKey; label: string }[] = [
  { key: 'sma', label: 'SMA 20' },
  { key: 'ema', label: 'EMA 50' },
  { key: 'rsi', label: 'RSI 14' },
]

// --- chart core -------------------------------------------------------------

let chart: IChartApi | null = null
let candles: ISeriesApi<'Candlestick'> | null = null
let volume: ISeriesApi<'Histogram'> | null = null
let smaLine: ISeriesApi<'Line'> | null = null
let emaLine: ISeriesApi<'Line'> | null = null
let rsiLine: ISeriesApi<'Line'> | null = null
let watermark: ITextWatermarkPluginApi<Time> | null = null
let drawings: DrawingManager | null = null

/** OHLCV in chart form; kept outside reactivity, the chart owns rendering. */
let bars: (BarData<UTCTimestamp> & { volume: number })[] = []
let precision = 2

let mounted = true
let ws: WebSocket | null = null
let wsRetry: ReturnType<typeof setTimeout> | null = null
let wsAttempts = 0

/** Guard so importJSON/clear do not feed back into persistence. */
let applying = false

const drawKey = computed(() => `tradeapp.drawings.${props.symbol}`)

const probe = document.createElement('canvas').getContext('2d')

/** Resolve a CSS variable to a color the canvas/LWC can parse (oklch-safe). */
function cssVar(name: string, fallback: string): string {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  if (!probe || !raw) return fallback
  probe.fillStyle = '#010203'
  probe.fillStyle = raw
  return probe.fillStyle === '#010203' ? fallback : probe.fillStyle
}

function alpha(color: string, a: number): string {
  if (color.startsWith('#')) {
    const hex = color.slice(1)
    const full = hex.length === 3 ? hex.split('').map((c) => c + c).join('') : hex
    const n = parseInt(full, 16)
    return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`
  }
  const m = color.match(/rgba?\(([^)]+)\)/)
  if (m) {
    const parts = m[1].split(/[,\s/]+/).filter(Boolean).map(Number)
    return `rgba(${parts[0]}, ${parts[1]}, ${parts[2]}, ${a})`
  }
  return color
}

function applyTheme() {
  if (!chart) return
  const dark = theme.value === 'dark'
  const bg = cssVar('--background', dark ? '#131722' : '#ffffff')
  const fg = cssVar('--foreground', dark ? '#d1d4dc' : '#131722')
  const border = cssVar('--border', dark ? '#2a2e39' : '#d1d4dc')
  const muted = cssVar('--muted-foreground', '#787b86')
  chart.applyOptions({
    layout: { background: { type: ColorType.Solid, color: bg }, textColor: fg },
    grid: {
      vertLines: { color: alpha(border, dark ? 0.55 : 0.8) },
      horzLines: { color: alpha(border, dark ? 0.55 : 0.8) },
    },
    crosshair: {
      vertLine: { color: alpha(muted, 0.6), labelBackgroundColor: '#2962ff' },
      horzLine: { color: alpha(muted, 0.6), labelBackgroundColor: '#2962ff' },
    },
    rightPriceScale: { borderColor: border },
    timeScale: { borderColor: border },
  })
  updateWatermark()
}

function updateWatermark() {
  if (!watermark) return
  const dark = theme.value === 'dark'
  const fg = cssVar('--foreground', dark ? '#d1d4dc' : '#131722')
  watermark.applyOptions({
    lines: [{ text: props.symbol, color: alpha(fg, 0.08), fontSize: 48 }],
  })
}

function priceFormatFor(p: number) {
  if (p >= 1000) return { type: 'price' as const, precision: 2, minMove: 0.01 }
  if (p >= 1) return { type: 'price' as const, precision: 4, minMove: 0.0001 }
  return { type: 'price' as const, precision: 8, minMove: 1e-8 }
}

function fmt(n: number): string {
  return n.toFixed(precision)
}

// --- indicators -------------------------------------------------------------

function smaArr(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = []
  let sum = 0
  for (let i = 0; i < values.length; i++) {
    sum += values[i]
    if (i >= period) sum -= values[i - period]
    out.push(i >= period - 1 ? sum / period : null)
  }
  return out
}

function emaArr(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = []
  const k = 2 / (period + 1)
  let prev: number | null = null
  for (let i = 0; i < values.length; i++) {
    if (i < period - 1) {
      out.push(null)
      continue
    }
    if (prev === null) {
      let seed = 0
      for (let j = i - period + 1; j <= i; j++) seed += values[j]
      prev = seed / period
      out.push(prev)
      continue
    }
    prev = values[i] * k + prev * (1 - k)
    out.push(prev)
  }
  return out
}

function rsiArr(values: number[], period: number): (number | null)[] {
  const out: (number | null)[] = new Array(values.length).fill(null)
  if (values.length <= period) return out
  let gain = 0
  let loss = 0
  for (let i = 1; i <= period; i++) {
    const d = values[i] - values[i - 1]
    if (d >= 0) gain += d
    else loss -= d
  }
  let ag = gain / period
  let al = loss / period
  const toRsi = () => (al === 0 ? 100 : 100 - 100 / (1 + ag / al))
  out[period] = toRsi()
  for (let i = period + 1; i < values.length; i++) {
    const d = values[i] - values[i - 1]
    ag = (ag * (period - 1) + Math.max(d, 0)) / period
    al = (al * (period - 1) + Math.max(-d, 0)) / period
    out[i] = toRsi()
  }
  return out
}

function lineData(values: (number | null)[]): { time: UTCTimestamp; value?: number }[] {
  return bars.map((b, i) =>
    values[i] === null || values[i] === undefined ? { time: b.time } : { time: b.time, value: values[i] as number },
  )
}

function toggleIndicator(key: IndicatorKey) {
  indicators.value[key] = !indicators.value[key]
}

function syncIndicators() {
  if (!bars.length) return
  const closes = bars.map((b) => b.close)
  if (smaLine) smaLine.setData(lineData(smaArr(closes, 20)))
  if (emaLine) emaLine.setData(lineData(emaArr(closes, 50)))
  if (rsiLine) rsiLine.setData(lineData(rsiArr(closes, 14)))
}

function setIndicatorSeries() {
  if (!chart || !candles) return
  if (indicators.value.sma && !smaLine) {
    smaLine = chart.addSeries(LineSeries, {
      color: '#f5a623',
      lineWidth: 1,
      priceLineVisible: false,
      lastValueVisible: false,
      crosshairMarkerVisible: false,
      priceScaleId: '',
    })
  } else if (!indicators.value.sma && smaLine) {
    chart.removeSeries(smaLine)
    smaLine = null
  }
  if (indicators.value.ema && !emaLine) {
    emaLine = chart.addSeries(LineSeries, {
      color: '#2962ff',
      lineWidth: 1,
      priceLineVisible: false,
      lastValueVisible: false,
      crosshairMarkerVisible: false,
      priceScaleId: '',
    })
  } else if (!indicators.value.ema && emaLine) {
    chart.removeSeries(emaLine)
    emaLine = null
  }
  if (indicators.value.rsi && !rsiLine) {
    const pane = chart.addPane()
    rsiLine = pane.addSeries(LineSeries, {
      color: '#9b6dff',
      lineWidth: 1,
      priceLineVisible: false,
      lastValueVisible: false,
      crosshairMarkerVisible: false,
      autoscaleInfoProvider: () => ({ priceRange: { minValue: 0, maxValue: 100 } }),
    })
    rsiLine.createPriceLine({
      price: 70,
      color: alpha('#787b86', 0.9),
      lineWidth: 1,
      lineStyle: LineStyle.Dashed,
      axisLabelVisible: false,
      title: '',
    })
    rsiLine.createPriceLine({
      price: 30,
      color: alpha('#787b86', 0.9),
      lineWidth: 1,
      lineStyle: LineStyle.Dashed,
      axisLabelVisible: false,
      title: '',
    })
    pane.setStretchFactor(0.3)
  } else if (!indicators.value.rsi && rsiLine) {
    chart?.removeSeries(rsiLine)
    rsiLine = null
  }
  syncIndicators()
}

// --- data -------------------------------------------------------------------

function setData(fit: boolean) {
  if (!candles || !volume) return
  empty.value = bars.length === 0
  if (!bars.length) {
    candles.setData([])
    volume.setData([])
    return
  }
  const last = bars[bars.length - 1]
  precision = priceFormatFor(last.close).precision
  candles.applyOptions({ priceFormat: priceFormatFor(last.close) })
  candles.setData(bars)
  volume.setData(
    bars.map((b) => ({
      time: b.time,
      value: b.volume,
      color: b.close >= b.open ? 'rgba(38,166,154,0.55)' : 'rgba(239,83,80,0.55)',
    })),
  )
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
    bars = out.candles.map((c) => ({
      time: (Math.floor(Date.parse(c.ts) / 1000)) as UTCTimestamp,
      open: Number(c.open),
      high: Number(c.high),
      low: Number(c.low),
      close: Number(c.close),
      volume: Number(c.volume),
    }))
    setData(true)
  } catch (e) {
    if (!mounted) return
    bars = []
    setData(false)
    error.value = parseError(e)
  } finally {
    if (mounted) loading.value = false
  }
}

// --- legend -----------------------------------------------------------------

function showBar(b: BarData<UTCTimestamp> & { volume?: number }) {
  legend.value = {
    time: new Date(Number(b.time) * 1000).toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    }),
    o: b.open,
    h: b.high,
    l: b.low,
    c: b.close,
    v: b.volume ?? 0,
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
      const k = msg.k
      if (!k) return
      stream.value = 'live'
      const time = (Math.floor(k.t / 1000)) as UTCTimestamp
      const bar = {
        time,
        open: Number(k.o),
        high: Number(k.h),
        low: Number(k.l),
        close: Number(k.c),
        volume: Number(k.v),
      }
      const last = bars[bars.length - 1]
      if (!last || time > last.time) bars.push(bar)
      else if (time === last.time) bars[bars.length - 1] = bar
      else return
      candles?.update(bar)
      volume?.update({
        time,
        value: bar.volume,
        color: bar.close >= bar.open ? 'rgba(38,166,154,0.55)' : 'rgba(239,83,80,0.55)',
      })
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
  drawings.on('add', (d) => {
    undoStack.value.push(d.id)
    drawingCount.value = drawings?.drawings().length ?? 0
    persist(drawKey.value)
  })
  drawings.on('remove', (d) => {
    undoStack.value = undoStack.value.filter((id) => id !== d.id)
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

function toggleTool(kind: DrawingKind) {
  if (!drawings) return
  const next = activeTool.value === kind ? null : kind
  drawings.setTool(next)
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

// --- lifecycle --------------------------------------------------------------

onMounted(() => {
  const el = chartEl.value
  if (!el) return
  chart = createChart(el, {
    autoSize: true,
    crosshair: { mode: CrosshairMode.Normal },
    rightPriceScale: { scaleMargins: { top: 0.08, bottom: 0.25 } },
    timeScale: { timeVisible: true, secondsVisible: false, rightOffset: 4 },
    localization: { locale: navigator.language },
  })
  candles = chart.addSeries(CandlestickSeries, {
    upColor: '#26a69a',
    downColor: '#ef5350',
    borderUpColor: '#26a69a',
    borderDownColor: '#ef5350',
    wickUpColor: '#26a69a',
    wickDownColor: '#ef5350',
  })
  const volPane = chart.addPane()
  volume = volPane.addSeries(HistogramSeries, {
    priceFormat: { type: 'volume' },
    priceLineVisible: false,
    lastValueVisible: false,
  })
  volPane.setStretchFactor(0.22)
  watermark = createTextWatermark(chart.panes()[0], {
    lines: [{ text: props.symbol, color: 'rgba(128,128,128,0.08)', fontSize: 48 }],
    horzAlign: 'center',
    vertAlign: 'center',
  })
  chart.subscribeCrosshairMove((param) => {
    const d = param.seriesData.get(candles!) as (BarData<UTCTimestamp> & { volume?: number }) | undefined
    if (d && param.time !== undefined) showBar(d)
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
  watermark = null
})

watch(theme, applyTheme)

watch(
  () => props.symbol,
  (_next, prev) => {
    if (prev) localStorage.setItem(`tradeapp.drawings.${prev}`, drawings?.exportJSON() ?? '[]')
    updateWatermark()
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

watch(indicators, setIndicatorSeries, { deep: true })

watch(magnet, (on) => drawings?.setMagnet(on ? 'weak' : 'off'))
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3">
    <div class="flex flex-wrap items-center gap-1">
      <Button
        v-for="m in TIMEFRAMES"
        :key="m"
        variant="ghost"
        size="sm"
        class="px-2 tabular-nums"
        :class="tf === m && 'bg-accent text-accent-foreground'"
        @click="tf = m"
      >
        {{ tfLabel(m) }}
      </Button>

      <div class="mx-2 h-5 w-px bg-border" />

      <Button
        v-for="ind in indicatorToggles"
        :key="ind.key"
        variant="ghost"
        size="sm"
        class="px-2 text-xs"
        :class="indicators[ind.key] && 'bg-accent text-accent-foreground'"
        @click="toggleIndicator(ind.key)"
      >
        {{ ind.label }}
      </Button>

      <div class="mx-2 h-5 w-px bg-border" />

      <Tooltip v-for="tool in tools" :key="tool.kind">
        <template #trigger>
          <Button
            variant="ghost"
            size="sm"
            class="px-2"
            :class="activeTool === tool.kind && 'bg-accent text-accent-foreground'"
            @click="toggleTool(tool.kind)"
          >
            <component :is="tool.icon" class="size-4" />
          </Button>
        </template>
        <span>{{ t(tool.key) }}</span>
      </Tooltip>

      <Tooltip>
        <template #trigger>
          <Button
            variant="ghost"
            size="sm"
            class="px-2"
            :class="magnet && 'bg-accent text-accent-foreground'"
            @click="magnet = !magnet"
          >
            <Magnet class="size-4" />
          </Button>
        </template>
        <span>{{ t('trading.magnet') }}</span>
      </Tooltip>

      <Tooltip>
        <template #trigger>
          <Button variant="ghost" size="sm" class="px-2" :disabled="!undoStack.length" @click="undoDrawing">
            <Undo2 class="size-4" />
          </Button>
        </template>
        <span>{{ t('trading.undo') }}</span>
      </Tooltip>

      <Tooltip>
        <template #trigger>
          <Button variant="ghost" size="sm" class="px-2" :disabled="!drawingCount" @click="clearDrawings">
            <Trash2 class="size-4" />
          </Button>
        </template>
        <span>{{ t('trading.clearDrawings') }}</span>
      </Tooltip>
    </div>

    <div class="relative min-h-0 flex-1 rounded-md border border-border bg-card">
      <div ref="chartEl" class="absolute inset-0" />

      <div
        v-if="legend"
        class="pointer-events-none absolute left-3 top-2 z-10 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-xs tabular-nums text-muted-foreground"
      >
        <span class="font-semibold text-foreground">{{ symbol }}</span>
        <span>{{ tfLabel(tf) }}</span>
        <span>{{ legend.time }}</span>
        <span>{{ t('trading.o') }} {{ fmt(legend.o) }}</span>
        <span>{{ t('trading.h') }} {{ fmt(legend.h) }}</span>
        <span>{{ t('trading.l') }} {{ fmt(legend.l) }}</span>
        <span :class="legend.c >= legend.o ? 'text-emerald-500' : 'text-red-500'">
          {{ t('trading.c') }} {{ fmt(legend.c) }}
        </span>
        <span :class="legend.c >= legend.o ? 'text-emerald-500' : 'text-red-500'">
          {{ legend.o ? (((legend.c - legend.o) / legend.o) * 100).toFixed(2) : '0.00' }}%
        </span>
        <span>{{ t('trading.vol') }} {{ Math.round(legend.v).toLocaleString() }}</span>
      </div>

      <div
        class="absolute right-3 top-2 z-10 flex items-center gap-1.5 rounded-full border border-border bg-card/80 px-2 py-0.5 text-xs"
      >
        <span
          class="size-1.5 rounded-full"
          :class="
            stream === 'live'
              ? 'bg-emerald-500'
              : stream === 'connecting'
                ? 'bg-amber-500'
                : 'bg-muted-foreground'
          "
        />
        {{ stream === 'live' ? t('trading.live') : stream === 'connecting' ? t('trading.connecting') : t('trading.down') }}
      </div>

      <div
        v-if="loading"
        class="absolute inset-0 z-20 flex items-center justify-center bg-card/60 text-sm text-muted-foreground"
      >
        {{ t('trading.loading') }}
      </div>
      <div
        v-else-if="error"
        class="absolute inset-0 z-20 flex items-center justify-center px-6 text-center text-sm text-destructive"
      >
        {{ error.message }}
      </div>
      <div
        v-else-if="empty"
        class="absolute inset-0 z-20 flex items-center justify-center px-6 text-center text-sm text-muted-foreground"
      >
        {{ t('trading.empty') }}
      </div>
    </div>
  </div>
</template>
