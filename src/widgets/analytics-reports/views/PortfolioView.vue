<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'

import { categoryChart, num } from './chart'

/**
 * Everything here comes from the report, so the period and wallets picked in
 * the filter bar above apply to all three charts (the dashboard widgets this
 * used to embed always showed the whole portfolio).
 */
const props = defineProps<{ report: PortfolioReport }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
const currency = computed(() => props.report.meta.currency)
const moneyTooltip = useMoneyChartTooltip(currency)
const labels = computed(() => props.report.points.map((point) => formatDate(point.date)))
const assetName = (symbol: string) => symbol || t('report.portfolio.other')

/** The portfolio's value over the period. */
const valueOption = computed<ChartOption>(() => {
  const base = categoryChart(labels.value, axisColor.value, moneyTooltip.value)
  return {
    ...base,
    legend: { show: false },
    yAxis: {
      ...(base.yAxis as object),
      axisLabel: { color: axisColor.value, formatter: (value: number) => formatMoney(value, { currency: currency.value, compact: true }) },
    },
    series: [
      {
        name: t('store.total'),
        type: 'line' as const,
        showSymbol: false,
        smooth: true,
        areaStyle: { opacity: 0.15 },
        data: props.report.points.map((point) => num(point.total)),
      },
    ],
  }
})

/** Share of each asset over time, stacked to 100 %. */
const shareOption = computed<ChartOption>(() => {
  const base = categoryChart(labels.value, axisColor.value)
  return {
    ...base,
    yAxis: { ...(base.yAxis as object), max: 100, axisLabel: { color: axisColor.value, formatter: '{value}%' } },
    series: props.report.assets.map((symbol, index) => ({
      name: assetName(symbol),
      type: 'line' as const,
      stack: 'share',
      areaStyle: { opacity: 0.5 },
      showSymbol: false,
      data: props.report.points.map((point) => num(point.shares[index])),
    })),
  }
})

/** Where the money sits at the end of the period, with the amount in the tooltip. */
const allocationOption = computed<ChartOption>(() => {
  const last = props.report.points.at(-1)
  return {
    backgroundColor: 'transparent',
    tooltip: {
      trigger: 'item',
      formatter: (params: unknown) => {
        const item = params as { name: string; value: number; percent: number }
        return `${item.name}: ${formatMoney(item.value, { currency: currency.value })} (${item.percent}%)`
      },
    },
    series: [
      {
        name: t('charts.distribution'),
        type: 'pie' as const,
        radius: ['45%', '72%'],
        itemStyle: { borderRadius: 6, borderColor: 'transparent', borderWidth: 2 },
        label: { color: axisColor.value, formatter: '{b}\n{d}%' },
        data: props.report.assets
          .map((symbol, index) => ({ name: assetName(symbol), value: num(last?.values[index]) }))
          .filter((slice) => slice.value > 0),
      },
    ],
  }
})
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @lg:grid-cols-[2fr_1fr]">
    <Card class="@lg:col-span-2">
      <template #header>
        <h3 class="heading-card">{{ t('charts.title') }}</h3>
      </template>
      <div class="h-72 w-full"><Chart :option="valueOption" autoresize /></div>
    </Card>
    <Card>
      <template #header>
        <h3 class="heading-card">{{ t('report.portfolio.title') }}</h3>
      </template>
      <div class="h-80 w-full"><Chart :option="shareOption" autoresize /></div>
    </Card>
    <Card>
      <template #header>
        <h3 class="heading-card">{{ t('charts.distribution') }}</h3>
      </template>
      <div class="h-80 w-full"><Chart :option="allocationOption" autoresize /></div>
    </Card>
  </div>
</template>
