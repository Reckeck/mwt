<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { Chart, provideChartTheme, type ChartOption } from '@/shared/lib/echarts'
import { call } from '@/shared/api/ipc'
import { useTheme } from '@/shared/lib/theme'

const { t } = useI18n()
const { resolved } = useTheme()
provideChartTheme(resolved.value)

const history = useQuery({
  queryKey: ['history'],
  queryFn: () => call<HistoryPoint[]>('get_history'),
})

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

const axisColor = computed(() => (resolved.value === 'dark' ? 'rgba(255,255,255,0.45)' : 'rgba(0,0,0,0.45)'))

const lineOption = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  tooltip: { trigger: 'axis' },
  grid: { left: 48, right: 24, top: 24, bottom: 32, containLabel: true },
  xAxis: {
    type: 'time',
    axisLabel: { color: axisColor.value },
    axisLine: { lineStyle: { color: axisColor.value } },
  },
  yAxis: {
    type: 'value',
    axisLabel: { color: axisColor.value },
    splitLine: { lineStyle: { color: axisColor.value, opacity: 0.15 } },
  },
  series: [
    {
      name: t('store.total'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      data: (history.data.value ?? []).map((point) => [Number(point.ts_ms), Number(point.value_base)]),
      areaStyle: { opacity: 0.12 },
    },
  ],
}))

const pieOption = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  tooltip: { trigger: 'item', formatter: '{b}: {d}%' },
  series: [
    {
      name: t('charts.distribution'),
      type: 'pie',
      radius: ['45%', '72%'],
      center: ['50%', '50%'],
      avoidLabelOverlap: true,
      itemStyle: { borderRadius: 6, borderColor: 'transparent', borderWidth: 2 },
      label: { color: axisColor.value, formatter: '{b}\n{d}%' },
      data: (summary.data.value?.by_chain ?? []).map((row) => ({
        name: row.label,
        value: Number(row.value_usd),
      })),
    },
  ],
}))
</script>

<template>
  <div class="grid gap-6 @container @md:grid-cols-2">
    <Card>
      <template #header>
        <p class="font-medium">{{ t('charts.title') }}</p>
      </template>
      <Chart
        v-if="(history.data.value ?? []).length > 1"
        :option="lineOption"
        autoresize
        class="h-80 w-full"
      />
      <p v-else class="py-12 text-center text-sm text-muted-foreground">{{ t('charts.historyEmpty') }}</p>
    </Card>

    <Card>
      <template #header>
        <p class="font-medium">{{ t('charts.distribution') }}</p>
      </template>
      <Chart
        v-if="(summary.data.value?.by_chain ?? []).length > 0"
        :option="pieOption"
        autoresize
        class="h-80 w-full"
      />
      <p v-else class="py-12 text-center text-sm text-muted-foreground">{{ t('store.empty') }}</p>
    </Card>
  </div>
</template>