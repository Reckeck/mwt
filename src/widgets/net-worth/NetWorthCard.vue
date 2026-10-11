<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { formatMoney } from '@/shared/lib/format'

const { t } = useI18n()

const worth = useQuery({ queryKey: ['net-worth'], queryFn: () => call<NetWorth>('get_net_worth') })
const history = useQuery({
  queryKey: ['report', 'networth', 'dashboard'],
  queryFn: () =>
    call<AnalyticsReport>('get_report', {
      kind: 'networth',
      filter: { scope: { store_ids: [], include_children: true }, from: null, to: null, year: null, currency: null, real_terms: false },
    }),
})
const points = computed(() => {
  const data = history.data.value
  return data && 'networth' in data ? data.networth.points : []
})

const rows = computed(() => {
  const data = worth.data.value
  if (!data) return []
  return [
    { key: 'financial', value: data.financial, sign: 1 },
    { key: 'non_financial', value: data.non_financial, sign: 1 },
    { key: 'receivables', value: data.receivables, sign: 1 },
    { key: 'liabilities', value: data.liabilities, sign: -1 },
    { key: 'payables', value: data.payables, sign: -1 },
  ].filter((row) => Number(row.value) !== 0)
})

/** A sparkline: no axes, no grid, just the shape of the last twelve months. */
const option = computed<ChartOption>(() => ({
  backgroundColor: 'transparent',
  grid: { left: 0, right: 0, top: 4, bottom: 0 },
  xAxis: { type: 'category', show: false, data: points.value.map((point) => point.month) },
  yAxis: { type: 'value', show: false, scale: true },
  tooltip: { trigger: 'axis' },
  series: [{ type: 'line', showSymbol: false, smooth: true, areaStyle: { opacity: 0.15 }, data: points.value.map((point) => Number(point.net)) }],
}))
</script>

<template>
  <Card size="sm" v-if="worth.data.value">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <RouterLink :to="{ name: 'analytics-networth' }" class="text-sm text-muted-foreground hover:text-foreground">
          {{ t('netWorthCard.title') }}
        </RouterLink>
        <span class="font-semibold tabular-nums">
          {{ formatMoney(worth.data.value.net, { decimals: 0, currency: worth.data.value.base_currency }) }}
        </span>
      </div>
    </template>
    <div class="grid gap-3">
      <div v-if="points.length > 1" class="h-16 w-full"><Chart :option="option" autoresize /></div>
      <dl class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs tabular-nums">
        <template v-for="row in rows" :key="row.key">
          <dt class="text-muted-foreground">{{ t(`report.networth.${row.key}`) }}</dt>
          <dd class="text-right" :class="row.sign < 0 && 'text-destructive'">
            {{ row.sign < 0 ? '−' : '' }}{{ formatMoney(row.value, { decimals: 0 }) }}
          </dd>
        </template>
      </dl>
    </div>
  </Card>
  <!-- Holds the cell while loading, so the widget does not pop in. -->
  <Card size="sm" v-else><Skeleton class="h-24 w-full" /></Card>
</template>
