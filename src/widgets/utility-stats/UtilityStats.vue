<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueries, useQuery } from '@tanstack/vue-query'
import { TrendingDown, TrendingUp } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatMoney } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { localDate } from '@/shared/lib/planning'
import { useTheme } from '@/shared/lib/theme'

import type { UtilityStatsProps } from './utility-stats.types'

/**
 * Utilities by month, stacked per bill, with the latest month against the one
 * before. Bills in another currency than the most common one are left off the
 * chart rather than added up as if they were the same money.
 */
const props = withDefaults(defineProps<UtilityStatsProps>(), { propertyId: undefined, months: 12 })
const { t } = useI18n()
const { resolved } = useTheme()
const axisColor = chartAxisColor(resolved)

const items = useQuery({ queryKey: ['utilities'], queryFn: () => call<UtilityItem[]>('list_utility_items') })
const own = computed(() =>
  (items.data.value ?? []).filter((item) => item.active && (!props.propertyId || item.property_id === props.propertyId)),
)
const trends = useQueries({
  queries: computed(() =>
    own.value.map((item) => ({
      queryKey: ['utility-trend', item.id],
      queryFn: () => call<UtilityTrend>('get_meter_trend', { itemId: item.id }),
    })),
  ),
})

/** The currency most bills are in; the chart speaks only that one. */
const currency = computed(() => {
  const counts = new Map<string, number>()
  for (const item of own.value) counts.set(item.currency, (counts.get(item.currency) ?? 0) + 1)
  return [...counts.entries()].sort((left, right) => right[1] - left[1])[0]?.[0] ?? ''
})

const periods = computed(() => {
  const now = new Date()
  return Array.from({ length: props.months }, (_unused, index) =>
    localDate(new Date(now.getFullYear(), now.getMonth() - (props.months - 1 - index), 1)).slice(0, 7),
  )
})

const series = computed(() =>
  own.value.flatMap((item, index) => {
    if (item.currency !== currency.value) return []
    const months = trends.value[index]?.data?.months ?? []
    const byPeriod = new Map(months.map((month) => [month.period, Number(month.amount)]))
    return [{ name: item.name, data: periods.value.map((period) => byPeriod.get(period) ?? 0) }]
  }),
)
const totals = computed(() =>
  periods.value.map((_period, index) => series.value.reduce((sum, row) => sum + (row.data[index] ?? 0), 0)),
)
/** The last month with any bill, and the one with a bill before it. */
const lastTwo = computed(() => {
  const filled = totals.value.map((value, index) => ({ value, index })).filter((entry) => entry.value > 0)
  return { last: filled.at(-1) ?? null, before: filled.at(-2) ?? null }
})
const deltaPct = computed(() => {
  const { last, before } = lastTwo.value
  if (!(last && before?.value)) return null
  return ((last.value - before.value) / before.value) * 100
})

const monthLabel = (period: string) =>
  new Intl.DateTimeFormat(activeLocale.value, { month: 'short' }).format(new Date(`${period}-01T12:00`))

const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  grid: { left: 4, right: 4, top: 8, bottom: 20, containLabel: true },
  tooltip: {
    trigger: 'axis',
    valueFormatter: (value) => formatMoney(Number(value), { currency: currency.value, decimals: 0 }),
  },
  xAxis: {
    type: 'category',
    data: periods.value.map(monthLabel),
    axisLabel: { color: axisColor.value, fontSize: 10 },
    axisTick: { show: false },
  },
  yAxis: { type: 'value', axisLabel: { color: axisColor.value, fontSize: 10 }, splitNumber: 3 },
  series: series.value.map((row) => ({ type: 'bar', stack: 'bill', name: row.name, data: row.data, barMaxWidth: 18 })),
}))
</script>

<template>
  <Card size="sm">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="text-sm text-muted-foreground">{{ t('utilityStats.title') }}</p>
        <span v-if="lastTwo.last" class="flex items-baseline gap-1.5 text-sm font-semibold">
          <Money :amount="lastTwo.last.value" :currency="currency" :decimals="0" />
          <span
            v-if="deltaPct !== null"
            class="inline-flex items-center gap-0.5 text-xs font-normal"
            :class="deltaPct > 0 ? 'text-destructive' : 'text-emerald-600 dark:text-emerald-400'"
          >
            <component :is="deltaPct > 0 ? TrendingUp : TrendingDown" class="size-3" />
            {{ deltaPct.toFixed(1) }}%
          </span>
        </span>
      </div>
    </template>
    <Skeleton v-if="items.isPending.value" class="h-40 w-full" />
    <p v-else-if="!own.length" class="text-sm text-muted-foreground">{{ t('utilities.noProperty') }}</p>
    <p v-else-if="!lastTwo.last" class="text-sm text-muted-foreground">{{ t('utilityStats.noBills') }}</p>
    <div v-else class="h-40 min-h-32 w-full"><Chart :option="option" autoresize /></div>
  </Card>
</template>
