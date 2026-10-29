<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money, useMoneyChartTooltip } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { chartColors } from '@/shared/lib/chart-theme'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { formatMoney } from '@/shared/lib/format'

/**
 * `{month} by day`: what was left at the end of each day, and dashed, where the
 * allowance takes it. The line carries its own labels (Money 3 · 15): the salary,
 * the biggest spending, today and where the month ends; the axis names 1/8/15/22/last.
 */
const props = defineProps<{ balance: MonthBalance; title: string; bare?: boolean }>()

const { t } = useI18n()
const categories = useCategories()
const tooltip = useMoneyChartTooltip(computed(() => props.balance.currency))
const amount = (value: string | null) => (value === null ? null : Number(value))

/** Axis days: the 1st, every seventh day after, and the last. */
const AXIS_STEP = 7

const option = computed<ChartOption>(() => {
  const days = props.balance.days
  const colors = chartColors()
  const code = props.balance.currency
  const money = (value: number, signed = false) =>
    formatMoney(value, { currency: code, decimals: 0, signed }).replace('-', '−')
  const indexOf = (date: string) => days.findIndex((day) => day.date === date)
  const label = (text: string, color: string, position: 'top' | 'bottom' | 'left' | 'right', bold = false, offset: [number, number] = [0, 0]) => ({
    show: true,
    formatter: text,
    position,
    offset,
    color,
    fontWeight: bold ? 700 : 400,
    fontSize: 11,
  })

  const fact: Array<number | null | Record<string, unknown>> = days.map((day) => amount(day.balance))
  const mark = (index: number, text: string, color: string, position: 'top' | 'bottom' | 'left' | 'right', bold = false, offset: [number, number] = [0, 0]) => {
    const value = fact[index]
    if (index < 0 || typeof value !== 'number') return
    fact[index] = { value, label: label(text, color, position, bold, offset), symbol: 'circle', symbolSize: bold ? 8 : 0 }
  }
  const since = props.balance.carry_since
  const carried = Number(props.balance.total.carried_in)
  if (since && carried) mark(0, `${money(carried)} ${t(`period.since.${since.slice(5, 7)}`)}`, colors.muted, 'bottom', false, [34, 4])
  const spent = props.balance.spent_mark
  if (spent) {
    const category = spent.category ? ` ${categoryKeyLabel(spent.category, categories.data.value, t)}` : ''
    mark(indexOf(spent.date), `−${money(Number(spent.amount))}${category}`, colors.muted, 'bottom')
  }
  const income = props.balance.income_mark
  if (income) {
    const category = income.category ? ` ${categoryKeyLabel(income.category, categories.data.value, t)}` : ''
    mark(indexOf(income.date), `+${money(Number(income.amount))}${category}`, colors.success, 'top')
  }
  const today = days.reduce((last, day, index) => (day.balance !== null ? index : last), -1)
  const todayValue = amount(days[today]?.balance ?? null)
  if (todayValue !== null) mark(today, t('balance.days.today', { sum: money(todayValue) }), colors.primary, 'bottom', true, [-44, 2])

  const forecast: Array<number | null | Record<string, unknown>> = days.map((day) => amount(day.forecast))
  const end = forecast.length - 1
  const endValue = typeof forecast[end] === 'number' ? (forecast[end] as number) : null
  if (endValue !== null) {
    forecast[end] = { value: endValue, label: label(`≈${money(endValue)}`, colors.muted, 'bottom', false, [-6, 2]), symbol: 'emptyCircle', symbolSize: 8 }
  }
  const ticks = new Set(days.map((_, index) => index).filter((index) => (index % AXIS_STEP === 0 && days.length - 1 - index >= 3) || index === days.length - 1))

  return {
    color: [colors.primary, colors.primary],
    tooltip: { trigger: 'axis', formatter: tooltip.value },
    grid: { left: 8, right: 36, top: 28, bottom: 8, containLabel: true },
    xAxis: {
      type: 'category',
      data: days.map((day) => String(Number(day.date.slice(8)))),
      boundaryGap: false,
      axisLabel: { interval: 0, formatter: (value: string, index: number) => (ticks.has(index) ? value : '') },
    },
    yAxis: { type: 'value' },
    series: [
      { name: t('balance.days.fact'), type: 'line', step: 'end', symbol: 'circle', symbolSize: 0, lineStyle: { width: 3 }, data: fact },
      { name: t('balance.days.forecast'), type: 'line', symbol: 'circle', symbolSize: 0, lineStyle: { width: 2, type: 'dashed' }, data: forecast },
    ],
  }
})
</script>

<template>
  <section :class="props.bare ? 'grid min-w-0' : 'grid min-w-0 gap-1 rounded-xl border bg-card px-4 py-3 shadow-sm'" data-tour="balance-days">
    <div v-if="!props.bare" class="flex flex-wrap items-baseline gap-x-2 text-sm">
      <b>{{ props.title }}</b>
      <span class="text-muted-foreground">{{ t('balance.days.hint') }}</span>
      <span class="flex-1" />
      <i18n-t v-if="props.balance.per_day" keypath="balance.days.legend" tag="span" class="text-muted-foreground max-md:hidden">
        <template #perDay><Money :amount="props.balance.per_day" :currency="props.balance.currency" :decimals="0" /></template>
      </i18n-t>
      <span v-else class="text-muted-foreground max-md:hidden">{{ t('balance.days.fact') }}</span>
    </div>
    <div class="h-52 w-full"><Chart :option="option" autoresize /></div>
  </section>
</template>
