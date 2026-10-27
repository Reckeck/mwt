<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useMoneyChartTooltip } from '@/entities/currency'
import { chartColors } from '@/shared/lib/chart-theme'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { formatMonthKey } from '@/shared/lib/format'

/**
 * «Перенесено дальше» as a line over the months (Money 3 · 15), shown while the
 * carry is on: a month that went into the red is an orange dot, the running month
 * is shaded. A click on a point opens that month.
 */
const props = defineProps<{ balance: MonthBalance }>()
const emit = defineEmits<{ pick: [month: string] }>()

const { t } = useI18n()
const tooltip = useMoneyChartTooltip(computed(() => props.balance.currency))

const option = computed<ChartOption>(() => {
  const rows = props.balance.rows
  const colors = chartColors()
  const last = rows.length - 1
  return {
    color: [colors.primary],
    tooltip: { trigger: 'axis', formatter: tooltip.value },
    grid: { left: 8, right: 8, top: 12, bottom: 4, containLabel: true },
    xAxis: { type: 'category', boundaryGap: false, data: rows.map((row) => formatMonthKey(row.month, { month: 'short' })), axisLabel: { show: false }, axisTick: { show: false } },
    yAxis: { type: 'value', splitNumber: 2 },
    series: [
      {
        name: t('balance.carried'),
        type: 'line',
        symbol: 'circle',
        symbolSize: 8,
        lineStyle: { width: 2 },
        data: rows.map((row) => ({
          value: Number(row.total),
          ...(Number(row.left) < 0 ? { itemStyle: { color: colors.warning } } : {}),
        })),
        markArea: rows[last]?.current
          ? { silent: true, itemStyle: { color: colors.border, opacity: 0.35 }, data: [[{ xAxis: last }, { xAxis: last }]] }
          : undefined,
      },
    ],
  }
})

function onClick(params: { dataIndex?: number }) {
  const row = props.balance.rows[params.dataIndex ?? -1]
  if (row) emit('pick', row.month)
}
</script>

<template>
  <div class="grid items-center gap-x-3 md:grid-cols-[110px_minmax(0,1fr)]" data-balance-carry-line>
    <p class="text-sm font-semibold max-md:hidden">{{ t('balance.carriedUnit', { currency: props.balance.currency.toUpperCase() }) }}</p>
    <div class="h-28 w-full"><Chart :option="option" @click="onClick" /></div>
  </div>
</template>
