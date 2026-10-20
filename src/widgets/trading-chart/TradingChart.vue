<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { BellPlus, Copy, PencilLine, Trash2 } from 'lucide-vue-next'

import { ContextMenu, type UiAction } from '@/shared/ui/context-menu'
import { copyText } from '@/shared/lib/clipboard'

import { minutesOf, tfLabel } from './lib/timeframe'
import { useBinanceStream } from './model/use-binance-stream'
import { useChartCore } from './model/use-chart-core'
import { useChartDrawings } from './model/use-chart-drawings'
import { DRAWING_TOOLS, type IndicatorKey } from './trading-chart.constants'
import type { TradingChartProps } from './trading-chart.types'
import { ChartLegend } from './ui/chart-legend'
import { ChartStatusOverlay } from './ui/chart-status-overlay'
import { ChartToolbar } from './ui/chart-toolbar'

/**
 * A candle chart of `symbol`/USDT from Binance with indicators, drawings and a
 * live stream. The chart lives in `model/use-chart-core`; drawings and the
 * stream hang off it; this file only wires them and the price menu.
 */
const props = withDefaults(defineProps<TradingChartProps>(), {
  limit: 500,
  canAlert: false,
  savedDrawings: () => ({}),
  timeframe: '1h',
  indicators: () => ['sma'],
})
const emit = defineEmits<{
  /** A symbol's drawings changed: the owner stores the export (an exchange terminal keeps it in the vault). */
  drawings: [symbol: string, json: string]
  timeframe: [label: string]
  indicators: [keys: IndicatorKey[]]
  /** «Alert at this price» from the chart's menu; the price is in USDT, the side is where it lies from the last close. */
  alert: [price: string, condition: 'above' | 'below']
}>()

const { t } = useI18n()

const tf = ref(minutesOf(props.timeframe))
const indicators = ref<IndicatorKey[]>([...props.indicators])
const chartEl = ref<HTMLDivElement | null>(null)

// Drawings and the stream are set up before the core: their teardown must run
// before the chart they draw on is removed (hooks run in registration order).
const drawings = useChartDrawings({
  symbol: () => props.symbol,
  saved: () => props.savedDrawings,
  tf,
  onChange: (symbol, json) => emit('drawings', symbol, json),
})
const stream = useBinanceStream(
  () => props.symbol,
  tf,
  (bar) => core.pushBar(bar),
)
const core = useChartCore({
  el: chartEl,
  symbol: () => props.symbol,
  limit: () => props.limit,
  tf,
  indicators,
  onReady: (handles) => {
    drawings.init(handles)
    stream.connect()
  },
})
const { loading, error, hasBars, legend, change, fmt } = core

watch(
  () => props.symbol,
  () => {
    drawings.load()
    void core.load()
    stream.restart()
  },
)
watch(tf, () => {
  emit('timeframe', tfLabel(tf.value))
  void core.load()
  stream.restart()
})
watch(indicators, (keys) => emit('indicators', [...keys]))

/* --- right-click menu: acts on the price under the cursor --- */
const cursorPrice = ref<number | null>(null)
function onChartMenu(event: MouseEvent) {
  const rect = chartEl.value?.getBoundingClientRect()
  const price = rect && core.handles.candles ? core.handles.candles.coordinateToPrice(event.clientY - rect.top) : null
  cursorPrice.value = price === null || !Number.isFinite(price) || price <= 0 ? null : price
}

/** From the toolbar: the last close, which the alert dialog lets the user change. */
function alertAtLast() {
  const last = core.handles.bars[core.handles.bars.length - 1]?.close
  if (last !== undefined) emit('alert', fmt(last), 'above')
}

const chartActions = computed<UiAction[]>(() => {
  const price = cursorPrice.value
  const shown = price === null ? '' : fmt(price)
  return [
    {
      id: 'alert',
      label: t('terminal.alertAt', { price: shown }),
      icon: BellPlus,
      hidden: !props.canAlert || price === null,
      run: () => {
        const last = core.handles.bars[core.handles.bars.length - 1]?.close ?? 0
        emit('alert', shown, price !== null && price < last ? 'below' : 'above')
      },
    },
    {
      id: 'draw',
      label: t('terminal.draw'),
      icon: PencilLine,
      children: DRAWING_TOOLS.map((tool) => ({
        id: `draw-${tool.kind}`,
        label: t(tool.key),
        icon: tool.icon,
        run: () => drawings.pickTool(tool.kind),
      })),
    },
    {
      id: 'copy',
      label: t('terminal.copyPrice', { price: shown }),
      icon: Copy,
      hidden: price === null,
      run: () => price !== null && void copyText(fmt(price), 'terminal.priceCopied'),
    },
    {
      id: 'clear',
      label: t('trading.clearDrawings'),
      icon: Trash2,
      destructive: true,
      separated: true,
      disabled: !drawings.count.value,
      run: drawings.clear,
    },
  ]
})
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-3">
    <ChartToolbar
      v-model:magnet="drawings.magnet.value"
      :tf="tf"
      :indicators="indicators"
      :active-tool="drawings.activeTool.value"
      :can-undo="drawings.undoStack.value.length > 0"
      :has-drawings="drawings.count.value > 0"
      :refreshing="loading && hasBars"
      :stream="stream.status.value"
      :can-alert="props.canAlert && hasBars"
      @alert="alertAtLast"
      @timeframe="(minutes) => (tf = minutes)"
      @indicators="(keys) => (indicators = keys)"
      @tool="drawings.pickTool"
      @undo="drawings.undo"
      @clear="drawings.clear"
    />

    <ContextMenu :actions="chartActions">
      <!-- The chart is a canvas the user works in (draws, scrolls, right-clicks): an application region. -->
      <div
        role="application"
        :aria-label="`${props.symbol} · ${tfLabel(tf)}`"
        class="relative min-h-0 flex-1 overflow-hidden rounded-lg border border-border bg-card"
        @contextmenu="onChartMenu"
      >
        <div ref="chartEl" class="absolute inset-0" />
        <ChartLegend v-if="legend" :legend="legend" :title="`${props.symbol} · ${tfLabel(tf)}`" :change="change" :fmt="fmt" />
        <ChartStatusOverlay v-if="!hasBars" :loading="loading" :error="error" @retry="core.load" />
      </div>
    </ContextMenu>
  </div>
</template>
