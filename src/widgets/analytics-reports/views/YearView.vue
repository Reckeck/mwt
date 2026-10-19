<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { monthName } from '@/entities/report'
import { formatPct } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { StatCard } from '@/shared/ui/stat-card'

import { transactionsOf } from '../lib/drill-down'
import { MonthTable, type MonthTableColumn, type MonthTableRow } from '../ui/month-table'
import { ReportDetails } from '../ui/report-details'
import { monthLabel } from '../use-report'

const props = defineProps<{ report: YearReport; filter: ReportFilter }>()

const { t } = useI18n()
const currency = computed(() => props.report.meta.currency)
const pct = (value: string) => formatPct(value, { decimals: 0 })

/** «Март 2026» for a card title. */
const monthTitle = (key: string) => {
  const name = monthName(key, activeLocale.value)
  return name.charAt(0).toUpperCase() + name.slice(1)
}

const rowOf = (month: string | null) => props.report.rows.find((row) => row.month === month) ?? null
const best = computed(() => rowOf(props.report.best_month))
/** One filled month is both best and worst: say it once. */
const worst = computed(() => (props.report.worst_month === props.report.best_month ? null : rowOf(props.report.worst_month)))

const columns = computed<MonthTableColumn[]>(() => [
  { key: 'income', label: t('report.cashflow.income'), hint: t('report.term.income'), primary: true },
  { key: 'savings_fact', label: t('report.year.savings_fact'), hint: t('report.term.savings_fact'), primary: true, signed: true },
  { key: 'saved_pct', label: '%', hint: t('report.term.saved_pct'), primary: true, format: pct },
  { key: 'fixed', label: t('report.year.fixed'), hint: t('report.term.fixed') },
  { key: 'daily', label: t('report.year.daily'), hint: t('report.term.daily') },
  { key: 'savings_plan', label: t('report.year.savings_plan'), hint: t('report.term.savings_plan') },
  { key: 'deviation', label: t('report.year.deviation'), hint: t('report.term.deviation'), signed: true },
])
const rows = computed<MonthTableRow[]>(() => [
  ...props.report.rows.map((row) => ({
    id: row.month,
    label: monthLabel(row.month, activeLocale.value),
    values: row as unknown as Record<string, string>,
    muted: row.empty,
    to: transactionsOf({ from: row.month }, props.filter),
  })),
  { id: 'total', label: t('report.year.total'), values: props.report.total as unknown as Record<string, string>, bold: true },
  {
    id: 'average',
    label: t('report.year.average'),
    values: props.report.average as unknown as Record<string, string>,
    bold: true,
  },
])
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-4">
      <StatCard :label="t('report.year.income', { year: props.report.year })">
        <Money :amount="props.report.total.income" :currency="currency" :decimals="0" />
      </StatCard>
      <StatCard :label="t('report.year.saved', { year: props.report.year })" :hint="pct(props.report.total.saved_pct)">
        <Money :amount="props.report.total.savings_fact" :currency="currency" :decimals="0" colored />
      </StatCard>
      <StatCard :label="t('report.year.best')" :value="best ? monthTitle(best.month) : '—'">
        <template v-if="best" #hint><Money :amount="best.savings_fact" :currency="currency" :decimals="0" colored /></template>
      </StatCard>
      <StatCard v-if="worst" :label="t('report.year.worst')" :value="monthTitle(worst.month)">
        <template #hint><Money :amount="worst.savings_fact" :currency="currency" :decimals="0" colored /></template>
      </StatCard>
    </div>
    <ReportDetails :label="t('report.byMonth')" default-open>
      <MonthTable :columns="columns" :rows="rows" :label-header="t('report.month')" :currency="props.report.meta.currency" />
      <p class="mt-3 text-xs text-muted-foreground">{{ t('report.year.averageNote') }}</p>
    </ReportDetails>
  </div>
</template>
