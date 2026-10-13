<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { ChartCurrencySwitch, useChartCurrency, useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { call } from '@/shared/api/ipc'
import { useTheme } from '@/shared/lib/theme'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const { resolved } = useTheme()
const axisColor = chartAxisColor(resolved)

const history = useQuery({
  queryKey: ['history'],
  queryFn: () => call<HistoryPoint[]>('get_history'),
})

/** History is in the base currency; the tooltip adds the favourite ones. */
const settings = useSettingsStore()
const shown = useChartCurrency(computed(() => settings.baseCurrency))
const moneyTooltip = useMoneyChartTooltip(shown.currency)

const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  tooltip: { trigger: 'axis', formatter: moneyTooltip.value },
  grid: { left: 4, right: 12, top: 24, bottom: 4, containLabel: true },
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
      data: (history.data.value ?? []).map((point) => [Number(point.ts_ms), shown.convert(Number(point.value_base))]),
      areaStyle: { opacity: 0.12 },
    },
  ],
}))
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('charts.title') }}</p>
        <ChartCurrencySwitch v-model="shown.selected.value" :options="shown.options.value" />
      </div>
    </template>
    <!-- The chart fills its box: vue-echarts forces height: 100% on its own element,
         so a height class on <Chart> is ignored and the canvas ends up 0px tall. -->
    <div v-if="(history.data.value ?? []).length > 1" class="h-80 w-full">
      <Chart :option="option" autoresize />
    </div>
    <p v-else class="py-12 text-center text-sm text-muted-foreground">{{ t('charts.historyEmpty') }}</p>
  </Card>
</template>
