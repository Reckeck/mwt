<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useMoneyChartTooltip } from '@/entities/currency'
import { axisNumber, chartColors } from '@/shared/lib/chart-theme'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { formatMoney, formatMonthKey } from '@/shared/lib/format'

import { STACKED, type WealthGroup, groupValue } from '../../lib/wealth-view'

/**
 * Capital by month as a stack: property under crypto under bank (Money 2 · 12).
 * Each layer is named at its right end in markup, as chart end labels overlap.
 */
const props = defineProps<{ points: NetworthPoint[]; currency: string }>()

const { t } = useI18n()
const tooltip = useMoneyChartTooltip(computed(() => props.currency))
const OPACITY: Record<string, number> = { property: 0.3, crypto: 0.8, bank: 0.15 }

const fill = computed<Record<string, string>>(() => {
  const colors = chartColors()
  return { property: colors.muted, crypto: colors.primary, bank: colors.popoverText }
})

const option = computed<ChartOption>(() => ({
  tooltip: { trigger: 'axis', formatter: tooltip.value },
  grid: { left: 8, right: 8, top: 16, bottom: 8, containLabel: true },
  xAxis: {
    type: 'category',
    boundaryGap: false,
    data: props.points.map((point) => formatMonthKey(point.month, { month: 'short' }).replace('.', '')),
  },
  yAxis: { type: 'value', axisLabel: { formatter: axisNumber } },
  series: STACKED.map((group) => ({
    name: t(`money.wealth.group.${group}`),
    type: 'line',
    stack: 'capital',
    showSymbol: false,
    color: fill.value[group],
    lineStyle: { width: group === 'bank' ? 2 : 0 },
    areaStyle: { opacity: OPACITY[group] },
    data: props.points.map((point) => Math.max(0, groupValue(point, group))),
  })),
}))

/** Top of the stack first, like the layers read on the chart. */
const labels = computed(() =>
  [...STACKED].reverse().map((group: WealthGroup) => {
    const last = props.points.at(-1)
    const value = last ? groupValue(last, group) : 0
    return { group, text: `${t(`money.wealth.group.${group}Short`)} ${formatMoney(value, { decimals: 0, currency: props.currency })}` }
  }),
)
</script>

<template>
  <div class="flex min-w-0 items-center gap-3 max-md:flex-col max-md:items-stretch">
    <div class="h-56 min-w-0 max-md:h-40 md:flex-1"><Chart :option="option" autoresize /></div>
    <ul class="grid shrink-0 gap-1.5 text-xs max-md:hidden">
      <li v-for="label in labels" :key="label.group" class="flex items-center gap-1.5 tabular-nums">
        <span class="size-2 rounded-full" :style="{ background: fill[label.group] }" aria-hidden="true" />
        {{ label.text }}
      </li>
    </ul>
  </div>
</template>
