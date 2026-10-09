<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { call } from '@/shared/api/ipc'
import { useTheme } from '@/shared/lib/theme'

const props = defineProps<{ storeId: string; baseCurrency: string }>()

const { t } = useI18n()
const { resolved } = useTheme()

const history = useQuery({
  queryKey: ['store-history', props.storeId],
  queryFn: () => call<HistoryPoint[]>('get_store_history', { storeId: props.storeId, baseCurrency: null }),
})

const points = computed(() => history.data.value ?? [])

const axisColor = chartAxisColor(resolved)

const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  tooltip: { trigger: 'axis' },
  grid: { left: 56, right: 24, top: 16, bottom: 32, containLabel: true },
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
      // ts_ms is a bigint on the TS side
      data: points.value.map((point) => [Number(point.ts_ms), Number(point.value_base)]),
      lineStyle: { width: 2 },
      areaStyle: { opacity: 0.12 },
    },
  ],
}))
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t('wallet.chart') }}</p>
    </template>

    <!-- The chart fills its box: vue-echarts forces height: 100% on its own element,
         so a height class on <Chart> is ignored and the canvas ends up 0px tall. -->
    <div v-if="points.length > 1" class="h-56 w-full">
      <Chart :option="option" autoresize />
    </div>
    <p v-else class="py-8 text-center text-sm text-muted-foreground">{{ t('wallet.chartEmpty') }}</p>
  </Card>
</template>
