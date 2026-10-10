<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { call, parseError } from '@/shared/api/ipc'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatDate, formatDateTime, formatMoney } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { EDGE_STYLES, KIND_COLORS, LANE_HEIGHT } from './tx-graph.constants'
import type { TxGraphViewProps } from './tx-graph.types'

const props = defineProps<TxGraphViewProps>()
const emit = defineEmits<{ open: [txId: string] }>()

const { t } = useI18n()
const { resolved } = useTheme()
const axisColor = chartAxisColor(resolved)

/** Stores whose sub-accounts are folded into their own lane. */
const collapsed = ref<string[]>([])

const graph = useQuery({
  queryKey: computed(() => ['transactions', 'graph', props.filter, collapsed.value]),
  queryFn: () => call<TxGraph>('get_tx_graph', { filter: props.filter, collapsed: collapsed.value }),
  placeholderData: (previous) => previous,
  retry: false,
})
const errorText = computed(() => {
  if (!graph.error.value) return ''
  const error = parseError(graph.error.value)
  return error.code === 'TOO_LARGE' ? t('graph.tooLarge') : codeErrorMessage(error, t)
})

const data = computed(() => graph.data.value ?? null)
const nodeById = computed(() => new Map((data.value?.nodes ?? []).map((node) => [node.id, node])))
/** The date under each column, for the x axis. */
const columnDates = computed(() => {
  const out = new Map<number, string>()
  for (const node of data.value?.nodes ?? []) if (!out.has(node.column)) out.set(node.column, node.timestamp)
  return out
})

/** A chain is lit while the pointer is on one of its operations. */
const hoveredFlow = ref<string | null>(null)

function laneLabel(lane: GraphLane): string {
  const name = lane.name.split(' › ').pop() ?? lane.name
  const fold = lane.has_children ? (lane.collapsed ? '▸ ' : '▾ ') : ''
  // Labels are right-aligned, so the nesting shows as a corner, not as an indent.
  return `${lane.depth ? '└ ' : ''}${fold}${name}`
}

function nodeSize(value: string): number {
  return Math.min(26, 8 + 4 * Math.log10(1 + Math.abs(Number(value) || 0)))
}

const kindsShown = computed(() => [...new Set((data.value?.nodes ?? []).map((node) => node.kind))])

const option = computed<ChartOption>(() => {
  const current = data.value
  if (!current) return {}
  const lit = hoveredFlow.value
  const dim = (flowId: string | null) => (lit && flowId !== lit ? 0.2 : 1)
  const visible = 80
  const start = current.columns > visible ? 100 - (visible / current.columns) * 100 : 0
  return {
    backgroundColor: 'transparent',
    animation: false,
    grid: { left: 8, right: 16, top: 12, bottom: 48, containLabel: true },
    tooltip: {
      trigger: 'item',
      confine: true,
      formatter: (params: unknown) => {
        const item = params as { dataType?: string; data?: { name?: string; edgeKind?: GraphEdgeKind } }
        if (item.dataType === 'edge') return t(`graph.edge.${item.data?.edgeKind ?? 'seq'}`)
        const node = nodeById.value.get(item.data?.name ?? '')
        if (!node) return ''
        const lines = [
          `<b>${t(`tx.kind.${node.kind}`)}</b> · ${formatDateTime(node.timestamp)}`,
          formatMoney(node.value, { currency: current.currency, decimals: 2 }),
        ]
        if (node.comment) lines.push(escapeHtml(node.comment))
        if (node.internal) lines.push(t('graph.internal'))
        if (node.flow_id) lines.push(t('graph.inFlow'))
        return lines.join('<br>')
      },
    },
    xAxis: {
      type: 'value',
      min: -0.5,
      max: Math.max(0.5, current.columns - 0.5),
      minInterval: 1,
      axisLabel: {
        color: axisColor.value,
        formatter: (value: number) => {
          const iso = columnDates.value.get(Math.round(value))
          return Number.isInteger(value) && iso ? formatDate(iso) : ''
        },
      },
      axisLine: { lineStyle: { color: axisColor.value } },
      splitLine: { show: false },
    },
    yAxis: {
      type: 'category',
      inverse: true,
      triggerEvent: true,
      data: current.lanes.map((_lane, index) => String(index)),
      axisLabel: {
        color: axisColor.value,
        formatter: (value: string) => {
          const lane = current.lanes[Number(value)]
          return lane ? laneLabel(lane) : ''
        },
      },
      axisTick: { show: false },
      splitLine: { show: true, lineStyle: { color: axisColor.value, opacity: 0.12 } },
    },
    dataZoom: [
      { type: 'inside', xAxisIndex: 0, filterMode: 'none', start },
      { type: 'slider', xAxisIndex: 0, filterMode: 'none', start, height: 16, bottom: 8 },
    ],
    series: [
      {
        type: 'graph',
        coordinateSystem: 'cartesian2d',
        layout: 'none',
        edgeSymbol: ['none', 'arrow'],
        edgeSymbolSize: 5,
        emphasis: { focus: 'none', scale: 1.4 },
        data: current.nodes.map((node) => ({
          name: node.id,
          value: [node.column, node.lane],
          symbolSize: nodeSize(node.value),
          itemStyle: {
            color: KIND_COLORS[node.kind],
            opacity: dim(node.flow_id),
            borderColor: node.flow_id ? EDGE_STYLES.flow.color : 'transparent',
            borderWidth: node.flow_id ? 2 : 0,
          },
        })),
        links: current.edges.map((edge) => {
          const style = EDGE_STYLES[edge.kind]
          const flowOf = nodeById.value.get(edge.source)?.flow_id ?? null
          return {
            source: edge.source,
            target: edge.target,
            edgeKind: edge.kind,
            lineStyle: {
              color: style.color,
              width: style.width,
              type: style.type,
              curveness: style.curveness,
              opacity: (edge.kind === 'seq' ? 0.5 : 0.85) * (lit && (edge.kind !== 'flow' || flowOf !== lit) ? 0.2 : 1),
            },
          }
        }),
      },
    ],
  } as ChartOption
})

function escapeHtml(text: string): string {
  return text.replace(/[&<>"']/g, (char) => `&#${char.charCodeAt(0)};`)
}

interface ChartEvent {
  componentType?: string
  dataType?: string
  value?: unknown
  data?: { name?: string } | null
}

function onClick(params: unknown) {
  const event = params as ChartEvent
  if (event.componentType === 'yAxis') {
    const lane = data.value?.lanes[Number(event.value)]
    if (!lane || (!lane.has_children && !lane.collapsed)) return
    collapsed.value = lane.collapsed
      ? collapsed.value.filter((id) => id !== lane.store_id)
      : [...collapsed.value, lane.store_id]
    return
  }
  if (event.dataType !== 'node') return
  const node = nodeById.value.get(event.data?.name ?? '')
  if (node) emit('open', node.tx_id)
}
function onHover(params: unknown) {
  const event = params as ChartEvent
  if (event.dataType !== 'node') return
  hoveredFlow.value = nodeById.value.get(event.data?.name ?? '')?.flow_id ?? null
}
function onLeave() {
  hoveredFlow.value = null
}

const height = computed(() => `${Math.max(3, data.value?.lanes.length ?? 0) * LANE_HEIGHT + 72}px`)
</script>

<template>
  <Card>
    <p v-if="errorText" class="py-12 text-center text-sm text-muted-foreground">{{ errorText }}</p>
    <p v-else-if="graph.isSuccess.value && !data?.nodes.length" class="py-12 text-center text-sm text-muted-foreground">
      {{ t('filter.empty') }}
    </p>
    <template v-else-if="data">
      <!-- vue-echarts forces height: 100% on its element, so the box sets the height. -->
      <div class="w-full" :style="{ height }">
        <Chart :option="option" autoresize @click="onClick" @mouseover="onHover" @mouseout="onLeave" />
      </div>
      <div class="mt-3 flex flex-wrap gap-x-4 gap-y-1.5 text-xs text-muted-foreground">
        <span v-for="kind in kindsShown" :key="kind" class="flex items-center gap-1.5">
          <span class="size-2.5 rounded-full" :style="{ background: KIND_COLORS[kind] }" />
          {{ t(`tx.kind.${kind}`) }}
        </span>
        <span v-for="kind in ['transfer', 'convert', 'flow'] as const" :key="kind" class="flex items-center gap-1.5">
          <span
            class="w-4 border-t-2"
            :class="EDGE_STYLES[kind].type === 'dashed' ? 'border-dashed' : ''"
            :style="{ borderColor: EDGE_STYLES[kind].color }"
          />
          {{ t(`graph.edge.${kind}`) }}
        </span>
      </div>
      <p class="mt-2 text-xs text-muted-foreground">{{ t('graph.hint') }}</p>
    </template>
  </Card>
</template>
