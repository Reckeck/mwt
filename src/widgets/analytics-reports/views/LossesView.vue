<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { LossSummary } from '@/widgets/loss-summary'
import { useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'

import { monthLabel } from '../use-report'
import { categoryChart, num } from './chart'

const props = defineProps<{ report: LossReport }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
/** Each value also in the base and favourite currencies. */
const moneyTooltip = useMoneyChartTooltip(computed(() => props.report.meta.currency))

const option = computed<ChartOption>(() => ({
  ...categoryChart(
    props.report.rows.map((row) => monthLabel(row.month, activeLocale.value)),
    axisColor.value,
    moneyTooltip.value,
  ),
  series: [
    { name: t('losses.fee'), type: 'bar', stack: 'loss', data: props.report.rows.map((row) => num(row.fee)) },
    { name: t('losses.slippage'), type: 'bar', stack: 'loss', data: props.report.rows.map((row) => num(row.slippage)) },
  ],
}))
</script>

<template>
  <div class="grid gap-4">
    <Card>
      <div class="h-72 w-full"><Chart :option="option" autoresize /></div>
    </Card>
    <LossSummary :report="props.report" />
  </div>
</template>
