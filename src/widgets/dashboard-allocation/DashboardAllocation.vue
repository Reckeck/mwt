<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { formatMoney, formatPct } from '@/shared/lib/format'
import { call } from '@/shared/api/ipc'

import type { DashboardAllocationProps } from './dashboard-allocation.types'

const props = defineProps<DashboardAllocationProps>()

const { t } = useI18n()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

/** One shape for both axes; only a store row links somewhere. */
const rows = computed(() => {
  const data = summary.data.value
  if (!data) return []
  return props.by === 'chain'
    ? data.by_chain.map((row) => ({ key: row.label, label: row.label, value: row.value_base, pct: row.pct, storeId: null }))
    : data.by_store.map((row) => ({
        key: row.store_id,
        label: row.name,
        value: row.value_base,
        pct: row.pct,
        storeId: row.store_id,
      }))
})

function pctWidth(value: string | number | null | undefined): string {
  const pct = typeof value === 'string' ? Number(value) : (value ?? 0)
  return `${Math.min(100, Math.max(0, pct)).toFixed(1)}%`
}
</script>

<template>
  <Card>
    <template #header>
      <p class="font-medium">{{ t(props.by === 'chain' ? 'store.byChain' : 'store.byStore') }}</p>
    </template>
    <div class="space-y-3">
      <component
        :is="row.storeId ? RouterLink : 'div'"
        v-for="row in rows"
        :key="row.key"
        :to="row.storeId ? { name: 'wallet', params: { id: row.storeId } } : undefined"
        class="flex flex-col gap-1"
        :class="row.storeId && 'rounded-md px-1 py-0.5 transition-colors hover:bg-accent'"
      >
        <div class="flex items-center justify-between text-sm">
          <span class="truncate">{{ row.label }}</span>
          <span class="shrink-0 text-muted-foreground">
            {{ formatMoney(row.value, { currency: summary.data.value?.base_currency }) }} · {{ formatPct(row.pct) }}
          </span>
        </div>
        <div class="h-2 overflow-hidden rounded-full bg-muted">
          <div class="h-full rounded-full bg-primary" :style="{ width: pctWidth(row.pct) }" />
        </div>
      </component>
      <p v-if="!rows.length" class="text-sm text-muted-foreground">{{ t('store.empty') }}</p>
    </div>
  </Card>
</template>
