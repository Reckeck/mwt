<script setup lang="ts">
import { computed } from 'vue'

import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { formatMonthKey } from '@/shared/lib/format'

/** The fetched months as a small line, the tooltip naming month and value. */
const props = defineProps<{ series: InflationReading[] }>()

const option = computed<ChartOption>(() => ({
  grid: { left: 2, right: 2, top: 4, bottom: 4 },
  xAxis: { type: 'category', show: false, data: props.series.map((point) => formatMonthKey(point.month)) },
  yAxis: { type: 'value', show: false, scale: true },
  tooltip: { trigger: 'axis', valueFormatter: (value) => `${value}%` },
  series: [{ type: 'line', data: props.series.map((point) => Number(point.yoy_pct)), showSymbol: false, smooth: true }],
}))
</script>

<template>
  <div class="h-10 w-28 shrink-0"><Chart :option="option" autoresize /></div>
</template>
