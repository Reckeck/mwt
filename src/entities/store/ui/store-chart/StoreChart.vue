<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { ChartCurrencySwitch, useChartCurrency, useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { call } from '@/shared/api/ipc'
import { currencySymbol, formatMoney } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'

import type { StoreChartProps } from './store-chart.types'

const props = withDefaults(defineProps<StoreChartProps>(), { includeChildren: true })

const { t } = useI18n()
const { resolved } = useTheme()

const history = useQuery({
  queryKey: computed(() => ['store-history', props.storeId, props.includeChildren]),
  queryFn: () =>
    call<HistoryPoint[]>('get_store_history', {
      storeId: props.storeId,
      includeChildren: props.includeChildren,
      baseCurrency: null,
    }),
})

const points = computed(() => history.data.value ?? [])

const axisColor = chartAxisColor(resolved)
/** The series comes in the base currency; the switch redraws it in a favourite one. */
const source = computed(() => (props.baseCurrency || 'usd').toLowerCase())
const shown = useChartCurrency(source)
const currency = shown.currency

const moneyTooltip = useMoneyChartTooltip(currency)

const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  // The value in the wallet's currency, then in the base and favourite ones.
  tooltip: { trigger: 'axis', formatter: moneyTooltip.value },
  // containLabel already reserves room for the axis labels; extra margins only add empty space.
  grid: { left: 4, right: 12, top: 12, bottom: 4, containLabel: true },
  xAxis: {
    type: 'time',
    axisLabel: { color: axisColor.value },
    axisLine: { lineStyle: { color: axisColor.value } },
  },
  yAxis: {
    type: 'value',
    axisLabel: {
      color: axisColor.value,
      formatter: (value: number) => formatMoney(value, { currency: currency.value, decimals: 0, compact: true }),
    },
    splitLine: { lineStyle: { color: axisColor.value, opacity: 0.15 } },
  },
  series: [
    {
      name: t('store.total'),
      type: 'line',
      smooth: true,
      showSymbol: false,
      // ts_ms is a bigint on the TS side
      data: points.value.map((point) => [Number(point.ts_ms), shown.convert(Number(point.value_base))]),
      lineStyle: { width: 2 },
      areaStyle: { opacity: 0.12 },
    },
  ],
}))
</script>

<template>
  <Card>
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('wallet.chart') }}</p>
        <ChartCurrencySwitch v-if="shown.options.value.length > 1" v-model="shown.selected.value" :options="shown.options.value" />
        <span v-else class="text-xs text-muted-foreground">{{ currencySymbol(currency) }}</span>
      </div>
    </template>

    <!-- The chart fills its box: vue-echarts forces height: 100% on its own element,
         so a height class on <Chart> is ignored and the canvas ends up 0px tall. -->
    <div v-if="points.length > 1" class="h-56 w-full">
      <Chart :option="option" autoresize />
    </div>
    <p v-else class="py-8 text-center text-sm text-muted-foreground">{{ t('wallet.chartEmpty') }}</p>
  </Card>
</template>
