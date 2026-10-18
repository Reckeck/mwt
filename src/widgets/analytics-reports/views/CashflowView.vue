<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'

import { Money, useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'
import { StatCard } from '@/shared/ui/stat-card'

import { transactionsOf } from '../lib/drill-down'
import { MonthTable, type MonthTableColumn, type MonthTableRow } from '../ui/month-table'
import { ReportDetails } from '../ui/report-details'
import { monthLabel } from '../use-report'
import { categoryChart, num } from './chart'

const props = defineProps<{ report: CashflowReport; filter: ReportFilter }>()

const { t } = useI18n()
const router = useRouter()
const axisColor = chartAxisColor(useTheme().resolved)
/** Each value also in the base and favourite currencies. */
const moneyTooltip = useMoneyChartTooltip(computed(() => props.report.meta.currency))
const currency = computed(() => props.report.meta.currency)

const option = computed<ChartOption>(() => ({
  ...categoryChart(
    props.report.rows.map((row) => monthLabel(row.month, activeLocale.value)),
    axisColor.value,
    moneyTooltip.value,
  ),
  series: [
    { name: t('report.cashflow.income'), type: 'bar', data: props.report.rows.map((row) => num(row.income)) },
    { name: t('report.cashflow.fixed'), type: 'bar', stack: 'out', data: props.report.rows.map((row) => num(row.fixed)) },
    { name: t('report.cashflow.daily'), type: 'bar', stack: 'out', data: props.report.rows.map((row) => num(row.daily)) },
    { name: t('report.cashflow.net'), type: 'line', data: props.report.rows.map((row) => num(row.net)) },
  ],
}))

/** A column of the chart opens that month's transactions. */
function openMonth(params: unknown) {
  const row = props.report.rows[(params as { dataIndex: number }).dataIndex]
  if (row) void router.push(transactionsOf({ from: row.month }, props.filter))
}

const spent = (row: CashflowRow) => String(num(row.fixed) + num(row.daily))

const columns = computed<MonthTableColumn[]>(() => [
  { key: 'income', label: t('report.cashflow.income'), hint: t('report.term.income'), primary: true },
  { key: 'spent', label: t('report.cashflow.spent'), hint: t('report.term.spent'), primary: true },
  { key: 'net', label: t('report.cashflow.net'), hint: t('report.term.net'), primary: true, signed: true },
  { key: 'fixed', label: t('report.cashflow.fixed'), hint: t('report.term.fixed') },
  { key: 'daily', label: t('report.cashflow.daily'), hint: t('report.term.daily') },
  { key: 'savings_moved', label: t('report.cashflow.savings_moved'), hint: t('report.term.savings_moved') },
  { key: 'budget_in', label: t('report.cashflow.budget_in'), hint: t('report.term.budget_in') },
  { key: 'budget_out', label: t('report.cashflow.budget_out'), hint: t('report.term.budget_out') },
])
const rows = computed<MonthTableRow[]>(() => [
  ...props.report.rows.map((row) => ({
    id: row.month,
    label: monthLabel(row.month, activeLocale.value),
    values: { ...row, spent: spent(row) },
    to: transactionsOf({ from: row.month }, props.filter),
  })),
  { id: 'total', label: t('report.total'), values: { ...props.report.total, spent: spent(props.report.total) }, bold: true },
])
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="grid gap-4 @sm:grid-cols-3">
      <StatCard :label="t('report.cashflow.income')">
        <Money :amount="props.report.total.income" :currency="currency" :decimals="0" />
      </StatCard>
      <StatCard :label="t('report.cashflow.spent')">
        <Money :amount="spent(props.report.total)" :currency="currency" :decimals="0" />
      </StatCard>
      <StatCard :label="t('report.cashflow.netTotal')">
        <Money :amount="props.report.total.net" :currency="currency" :decimals="0" colored />
      </StatCard>
    </div>
    <Card>
      <div class="h-80 w-full"><Chart :option="option" autoresize class="cursor-pointer" @click="openMonth" /></div>
    </Card>
    <ReportDetails :label="t('report.byMonth')">
      <MonthTable :columns="columns" :rows="rows" :label-header="t('report.month')" />
      <p class="mt-3 text-xs text-muted-foreground">
        {{ t('report.currencyNote', { currency: currency.toUpperCase() }) }}
        <template v-if="props.report.meta.real_terms">· {{ t('report.realNote', { rate: props.report.meta.inflation_rate ?? '—' }) }}</template>
      </p>
    </ReportDetails>
  </div>
</template>
