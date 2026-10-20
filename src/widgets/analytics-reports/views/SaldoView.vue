<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { Money, useMoneyChartTooltip } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatDay } from '@/shared/lib/format'
import { useTheme } from '@/shared/lib/theme'
import { buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { StatCard } from '@/shared/ui/stat-card'

import { MonthTable, type MonthTableColumn, type MonthTableRow } from '../ui/month-table'
import { ReportDetails } from '../ui/report-details'
import { num } from './chart'
import SaldoPool from './SaldoPool.vue'
import { type SaldoLabels, balanceChart, spendingChart, transactionsOfDay } from './saldo'

/**
 * The month's running balance day by day: the pool (income − planned mandatory −
 * savings), what everyday spending left of it, the forecast ahead, and the
 * transactions behind each day. The same figures as the calendar's.
 */
const props = defineProps<{ month: string }>()

const { t } = useI18n()
const router = useRouter()
const axisColor = chartAxisColor(useTheme().resolved)

const calendar = useQuery({
  queryKey: computed(() => ['calendar', props.month]),
  placeholderData: keepPreviousData,
  queryFn: () => call<CalendarMonth>('get_calendar', { month: props.month }),
})
const data = computed(() => calendar.data.value ?? null)
const currency = computed(() => data.value?.currency ?? '')
const moneyTooltip = useMoneyChartTooltip(currency)

const labelOf = (day: CalendarDay) => formatDay(`${day.date}T12:00`)
const labels = computed(() => (data.value?.days ?? []).map((day) => Number(day.date.slice(8)).toString()))
const names = computed<SaldoLabels>(() => ({
  left: t('saldo.left'),
  forecast: t('saldo.forecast'),
  even: t('saldo.even'),
  daily: t('saldo.daily'),
  limit: t('saldo.limit'),
}))
const balance = computed<ChartOption>(() =>
  data.value ? balanceChart(data.value, labels.value, axisColor.value, moneyTooltip.value, names.value) : {},
)
const spending = computed<ChartOption>(() =>
  data.value ? spendingChart(data.value, labels.value, axisColor.value, moneyTooltip.value, names.value) : {},
)

/** The rest as of today (or the month's last day already past). */
const latest = computed(() => [...(data.value?.days ?? [])].reverse().find((day) => !day.forecast) ?? null)
const dailySoFar = computed(() =>
  String((data.value?.days ?? []).filter((day) => !day.forecast).reduce((sum, day) => sum + num(day.daily), 0)),
)

/** A point of either chart opens that day's transactions. */
function openDay(params: unknown) {
  const day = data.value?.days[(params as { dataIndex: number }).dataIndex]
  if (day && day.tx_count > 0) void router.push(transactionsOfDay(day.date))
}

const columns = computed<MonthTableColumn[]>(() => [
  // Two figures fit a phone; the rest open under the row there.
  { key: 'daily', label: t('saldo.spent'), hint: t('saldo.hint.daily'), primary: true },
  { key: 'cumulative', label: t('saldo.cumulative'), hint: t('saldo.hint.cumulative') },
  { key: 'left', label: t('saldo.left'), hint: t('saldo.hint.left'), primary: true, signed: true },
  { key: 'days_ahead', label: t('saldo.daysAhead'), hint: t('saldo.hint.daysAhead'), format: (value) => value },
  { key: 'per_day_left', label: t('saldo.perDay'), hint: t('saldo.hint.perDay') },
  { key: 'fixed', label: t('saldo.fixed'), hint: t('saldo.hint.fixed') },
  { key: 'income', label: t('saldo.income') },
  { key: 'tx_count', label: t('saldo.operations'), format: (value) => value || '—' },
])
const rows = computed<MonthTableRow[]>(() =>
  (data.value?.days ?? []).map((day, index, days) => ({
    id: day.date,
    label: day.forecast ? `≈ ${labelOf(day)}` : labelOf(day),
    values: {
      daily: day.forecast ? '' : day.daily,
      // Left = the reserve − spent since the 1st, so the cumulative is what the rest leaves out.
      cumulative: day.left === null ? '' : String(num(data.value?.budget) - num(day.left)),
      left: day.left ?? '',
      days_ahead: String(days.length - index - 1),
      per_day_left: day.per_day_left ?? '',
      fixed: day.forecast ? '' : String(num(day.spent) - num(day.daily)),
      income: day.forecast ? '' : day.income,
      tx_count: day.tx_count ? String(day.tx_count) : '',
    },
    to: day.tx_count > 0 ? transactionsOfDay(day.date) : undefined,
  })),
)
</script>

<template>
  <SectionState
    :loading="calendar.isPending.value"
    :error="calendar.error.value"
    :empty="data !== null && data.budget === null"
    :empty-title="t('saldo.empty.title')"
    :empty-description="t('saldo.empty.description')"
    skeleton="report"
    @retry="calendar.refetch()"
  >
    <template #empty>
      <RouterLink :to="{ name: 'planning-budgets' }" :class="buttonVariants()">{{ t('saldo.empty.action') }}</RouterLink>
    </template>
    <div v-if="data && data.budget !== null" class="grid grid-cols-1 gap-4">
      <div class="grid grid-cols-2 gap-3 @wide:grid-cols-4 @wide:gap-4">
        <StatCard :label="t('saldo.reserve')" :hint="t('saldo.hint.pool')">
          <Money :amount="data.budget" :currency="currency" :decimals="0" />
        </StatCard>
        <StatCard :label="t('saldo.dailySoFar')">
          <Money :amount="dailySoFar" :currency="currency" :decimals="0" />
        </StatCard>
        <StatCard :label="t('saldo.leftNow')">
          <Money
            :amount="latest?.left ?? '0'"
            :currency="currency"
            :decimals="0"
            :class="Number(latest?.left) < 0 && 'text-destructive'"
          />
        </StatCard>
        <StatCard :label="t('saldo.perDay')">
          <Money :amount="latest?.per_day_left ?? '0'" :currency="currency" :decimals="0" />
        </StatCard>
      </div>
      <SaldoPool :month="data" :spent="dailySoFar" :latest="latest" />
      <Card>
        <template #header>
          <p class="font-medium">{{ t('saldo.chart.balance') }}</p>
        </template>
        <div class="h-72 w-full"><Chart :option="balance" autoresize class="cursor-pointer" @click="openDay" /></div>
      </Card>
      <Card>
        <template #header>
          <p class="font-medium">{{ t('saldo.chart.spending') }}</p>
        </template>
        <div class="h-64 w-full"><Chart :option="spending" autoresize class="cursor-pointer" @click="openDay" /></div>
      </Card>
      <ReportDetails :label="t('saldo.byDay')" default-open>
        <MonthTable :columns="columns" :rows="rows" :label-header="t('saldo.day')" :currency="currency" />
        <p class="mt-3 text-xs text-muted-foreground">{{ t('saldo.note') }}</p>
      </ReportDetails>
      <p class="text-xs text-muted-foreground">
        {{ t('report.currencyNote', { currency: currency.toUpperCase() }) }} ·
        <RouterLink :to="{ name: 'transactions-calendar' }" class="underline-offset-4 hover:underline">
          {{ t('saldo.toCalendar') }}
        </RouterLink>
      </p>
    </div>
  </SectionState>
</template>
