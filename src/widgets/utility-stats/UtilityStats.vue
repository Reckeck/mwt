<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQueries, useQuery } from '@tanstack/vue-query'
import { TrendingDown, TrendingUp } from 'lucide-vue-next'

import { ChartCurrencySwitch, Money, useChartCurrency, useConversions, useMoneyChartTooltip } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { formatMonthKey, formatPct } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { useTheme } from '@/shared/lib/theme'

import type { UtilityStatsProps } from './utility-stats.types'
import { SectionState } from '@/shared/ui/section-state'

/**
 * Utilities by month, stacked per bill, with the latest month against the one
 * before. The chart speaks the currency most bills are in; a bill in another
 * one is converted at the cached rate (marked «≈»), never added up as is, and
 * left off only while its rate is unknown.
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

const conversions = useConversions(computed(() => own.value.some((item) => item.currency !== currency.value)))
/** The bills' currency, or the base or a favourite one the user switches to. */
const shown = useChartCurrency(currency)
const moneyTooltip = useMoneyChartTooltip(shown.currency)

const series = computed(() =>
  own.value.flatMap((item, index) => {
    const foreign = item.currency !== currency.value
    const factor = foreign ? conversions.convertTo(1, item.currency, currency.value) : 1
    if (factor === null) return []
    const months = trends.value[index]?.data?.months ?? []
    const byPeriod = new Map(months.map((month) => [month.period, shown.convert(Number(month.amount) * factor)]))
    return [
      {
        name: foreign ? `${item.name} (≈ ${item.currency.toUpperCase()})` : item.name,
        data: periods.value.map((period) => byPeriod.get(period) ?? 0),
      },
    ]
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

const monthLabel = (period: string) => formatMonthKey(period, { month: 'short' })

const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  grid: { left: 4, right: 4, top: 8, bottom: 20, containLabel: true },
  tooltip: { trigger: 'axis', formatter: moneyTooltip.value },
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
        <p data-widget-title class="text-sm text-muted-foreground">{{ t('utilityStats.title') }}</p>
        <span v-if="lastTwo.last" class="flex items-baseline gap-1.5 text-sm font-semibold">
          <Money :amount="lastTwo.last.value" :currency="shown.currency.value" :decimals="0" />
          <span
            v-if="deltaPct !== null"
            class="inline-flex items-center gap-0.5 text-xs font-normal"
            :class="deltaPct > 0 ? 'text-destructive' : 'text-success'"
          >
            <component :is="deltaPct > 0 ? TrendingUp : TrendingDown" class="size-3" />
            {{ formatPct(deltaPct, { signed: true, decimals: 1 }) }}
          </span>
        </span>
      </div>
    </template>
    <Skeleton v-if="items.isPending.value" class="h-40 w-full" />
    <SectionState v-else-if="items.error.value" :error="items.error.value" compact @retry="items.refetch()" />
    <p v-else-if="!own.length" class="text-sm text-muted-foreground">{{ t('utilities.noProperty') }}</p>
    <p v-else-if="!lastTwo.last" class="text-sm text-muted-foreground">{{ t('utilityStats.noBills') }}</p>
    <template v-else>
      <ChartCurrencySwitch v-model="shown.selected.value" :options="shown.options.value" class="mb-2 justify-self-start" />
      <div class="h-40 min-h-32 w-full"><Chart :option="option" autoresize /></div>
    </template>
  </Card>
</template>
