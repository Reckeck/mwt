<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useMoneyChartTooltip } from '@/entities/currency'
import { chartColors } from '@/shared/lib/chart-theme'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { formatMoney } from '@/shared/lib/format'

/** «{Month} по дням»: what was left at the end of each day, and dashed, where the allowance takes it. */
const props = defineProps<{ balance: MonthBalance; title: string }>()

const { t } = useI18n()
const tooltip = useMoneyChartTooltip(computed(() => props.balance.currency))
const amount = (value: string | null) => (value === null ? null : Number(value))

const option = computed<ChartOption>(() => {
  const days = props.balance.days
  const color = chartColors().primary
  return {
    color: [color, color],
    tooltip: { trigger: 'axis', formatter: tooltip.value },
    grid: { left: 8, right: 16, top: 12, bottom: 8, containLabel: true },
    xAxis: { type: 'category', data: days.map((day) => String(Number(day.date.slice(8)))), boundaryGap: false },
    yAxis: { type: 'value' },
    series: [
      { name: t('balance.days.fact'), type: 'line', step: 'end', showSymbol: false, lineStyle: { width: 3 }, data: days.map((day) => amount(day.balance)) },
      {
        name: t('balance.days.forecast'),
        type: 'line',
        showSymbol: false,
        lineStyle: { width: 2, type: 'dashed' },
        data: days.map((day) => amount(day.forecast)),
      },
    ],
  }
})
const legend = computed(() =>
  props.balance.per_day
    ? t('balance.days.legend', { perDay: formatMoney(props.balance.per_day, { decimals: 0, currency: props.balance.currency }) })
    : t('balance.days.fact'),
)
</script>

<template>
  <section class="grid min-w-0 gap-1 rounded-xl border bg-card px-4 py-3 shadow-sm" data-tour="balance-days">
    <div class="flex flex-wrap items-baseline gap-x-2 text-sm">
      <b>{{ props.title }}</b>
      <span class="text-muted-foreground">{{ t('balance.days.hint') }}</span>
      <span class="flex-1" />
      <span class="text-muted-foreground max-md:hidden">{{ legend }}</span>
    </div>
    <div class="h-44 w-full"><Chart :option="option" autoresize /></div>
  </section>
</template>
