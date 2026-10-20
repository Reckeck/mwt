<script setup lang="ts">
import { computed } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money, PnlText, useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { StatCard } from '@/shared/ui/stat-card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'

import { transactionsOf } from '../lib/drill-down'
import { monthLabel } from '../use-report'
import { categoryChart, num } from './chart'

const props = defineProps<{ report: PnlReport; filter: ReportFilter }>()

const { t } = useI18n()
const router = useRouter()

/** A bar opens that month's transactions: the sales behind the result, or the fees. */
function openMonth(params: unknown) {
  const { dataIndex, seriesIndex } = params as { dataIndex: number; seriesIndex: number }
  const row = props.report.rows[dataIndex]
  if (!row) return
  const target = seriesIndex === 0 ? { realized: true } : { withFee: true }
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
    { name: t('report.pnl.realized'), type: 'bar', data: props.report.rows.map((row) => num(row.realized)) },
    { name: t('report.pnl.fees'), type: 'bar', data: props.report.rows.map((row) => -num(row.fees)) },
  ],
}))
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="grid gap-4 @xl:grid-cols-2">
      <StatCard :label="t('report.pnl.realized')">
        <PnlText :amount="props.report.total_realized" :currency="props.report.meta.currency" :decimals="2" />
      </StatCard>
      <StatCard :label="t('report.pnl.fees')">
        <Money :amount="props.report.total_fees" :currency="props.report.meta.currency" :decimals="2" />
      </StatCard>
    </div>
    <Card>
      <div class="h-72 w-full"><Chart :option="option" autoresize class="cursor-pointer" @click="openMonth" /></div>
    </Card>
  </div>
</template>
