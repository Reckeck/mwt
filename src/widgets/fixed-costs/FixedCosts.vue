<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQueries, useQuery } from '@tanstack/vue-query'
import { CircleAlert } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { Card } from '@/shared/ui/card'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { SectionState } from '@/shared/ui/section-state'

const { t } = useI18n()

const items = useQuery({ queryKey: ['utilities'], queryFn: () => call<UtilityItem[]>('list_utility_items') })
const active = computed(() => (items.data.value ?? []).filter((item) => item.active))
const trends = useQueries({
  queries: computed(() =>
    active.value.map((item) => ({
      queryKey: ['utility-trend', item.id],
      queryFn: () => call<UtilityTrend>('get_meter_trend', { itemId: item.id }),
    })),
  ),
})

const rows = computed(() =>
  active.value.map((item, index) => {
    const trend = trends.value[index]?.data
    return { item, amount: trend?.forecast ?? trend?.monthly ?? '0', anomaly: trend?.anomaly ?? false }
  }),
)
/** Bills can be in different currencies: a total per currency. */
const totals = computed(() => {
  const sums = new Map<string, number>()
  for (const row of rows.value) sums.set(row.item.currency, (sums.get(row.item.currency) ?? 0) + Number(row.amount))
  return [...sums.entries()]
})
</script>

<template>
  <Card size="sm" v-if="active.length">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <span data-widget-title class="text-sm text-muted-foreground">{{ t('utilities.dashboardTitle') }}</span>
        <span class="text-sm tabular-nums">
          {{ totals.map(([currency, sum]) => formatMoney(sum, { decimals: 0, currency })).join(' + ') }}
        </span>
      </div>
    </template>
    <ul class="grid gap-1.5 text-sm">
      <li v-for="row in rows" :key="row.item.id" class="flex items-center justify-between gap-2">
        <!-- A bill lives in its property: the row opens that property's utilities. -->
        <RouterLink
          :to="{ name: 'wallet', params: { id: row.item.property_id }, query: { tab: 'utilities' } }"
          :data-entity="`store:${row.item.property_id}`"
          class="flex min-w-0 items-center gap-1.5 truncate hover:underline"
        >
          <CircleAlert v-if="row.anomaly" class="size-3.5 shrink-0 text-destructive" :aria-label="t('utilities.anomaly')" />
          {{ row.item.name }}
        </RouterLink>
        <span class="shrink-0 tabular-nums text-muted-foreground">
          <Money :amount="row.amount" :decimals="0" :currency="row.item.currency" />
        </span>
      </li>
    </ul>
    <p class="mt-2 text-xs text-muted-foreground">{{ t('utilities.dashboardHint') }}</p>
  </Card>
  <Card size="sm" v-else-if="items.isPending.value"><Skeleton class="h-24 w-full" /></Card>
  <Card size="sm" v-else-if="items.error.value">
    <SectionState :error="items.error.value" compact @retry="items.refetch()" />
  </Card>
  <Card size="sm" v-else>
    <p class="text-sm text-muted-foreground">{{ t('dashboard.widgetEmpty') }}</p>
  </Card>
</template>
