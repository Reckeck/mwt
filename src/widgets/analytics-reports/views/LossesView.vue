<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { LossSummary } from '@/widgets/loss-summary'
import { useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'

import { transactionsOf } from '../lib/drill-down'
import { monthLabel } from '../use-report'
import { categoryChart, num } from './chart'

const props = defineProps<{ report: LossReport; filter: ReportFilter }>()

const { t } = useI18n()
const router = useRouter()

/** A bar opens that month's transactions: those that paid a fee, or the conversions behind the spread. */
function openMonth(params: unknown) {
  const { dataIndex, seriesIndex } = params as { dataIndex: number; seriesIndex: number }
  const row = props.report.rows[dataIndex]
  if (!row) return
  const target = seriesIndex === 0 ? { withFee: true } : { kinds: ['convert' as TxKind] }
  void router.push(transactionsOf({ from: row.month, ...target }, props.filter))
}
const axisColor = chartAxisColor(useTheme().resolved)
/** Each value also in the base and favourite currencies. */
const moneyTooltip = useMoneyChartTooltip(computed(() => props.report.meta.currency))

const option = computed<ChartOption>(() => ({
  ...categoryChart(
    props.report.rows.map((row) => monthLabel(row.month, activeLocale.value)),
    axisColor.value,
    moneyTooltip.value,
    props.report.meta.currency,
  ),
  series: [
    { name: t('losses.fee'), type: 'bar', stack: 'loss', data: props.report.rows.map((row) => num(row.fee)) },
    { name: t('losses.slippage'), type: 'bar', stack: 'loss', data: props.report.rows.map((row) => num(row.slippage)) },
  ],
}))
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <Card>
      <div class="h-72 w-full"><Chart :option="option" autoresize class="cursor-pointer" @click="openMonth" /></div>
    </Card>
    <LossSummary :report="props.report" />
  </div>
</template>
