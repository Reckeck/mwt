<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatMoney } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { useTheme } from '@/shared/lib/theme'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { monthLabel } from '../use-report'
import { categoryChart, num } from './chart'

const props = defineProps<{ report: CashflowReport }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
/** Each value also in the base and favourite currencies. */
const moneyTooltip = useMoneyChartTooltip(computed(() => props.report.meta.currency))
const money = (value: string) => formatMoney(value, { decimals: 0 })

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

const columns = ['income', 'fixed', 'daily', 'savings_moved', 'budget_in', 'budget_out', 'net'] as const
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <Card>
      <div class="h-80 w-full"><Chart :option="option" autoresize /></div>
    </Card>
    <Card>
      <div class="overflow-x-auto">
        <Table class="w-full text-sm tabular-nums">
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead class="text-left">{{ t('report.month') }}</TableHead>
              <TableHead v-for="column in columns" :key="column" class="text-right">
                {{ t(`report.cashflow.${column}`) }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="row in props.report.rows" :key="row.month">
              <TableCell>{{ monthLabel(row.month, activeLocale) }}</TableCell>
              <TableCell v-for="column in columns" :key="column" class="text-right">{{ money(row[column]) }}</TableCell>
            </TableRow>
            <TableRow class="font-medium">
              <TableCell>{{ t('report.total') }}</TableCell>
              <TableCell v-for="column in columns" :key="column" class="text-right">
                {{ money(props.report.total[column]) }}
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </div>
      <p class="mt-3 text-xs text-muted-foreground">
        {{ t('report.currencyNote', { currency: props.report.meta.currency.toUpperCase() }) }}
        <template v-if="props.report.meta.real_terms">· {{ t('report.realNote', { rate: props.report.meta.inflation_rate ?? '—' }) }}</template>
      </p>
    </Card>
  </div>
</template>
