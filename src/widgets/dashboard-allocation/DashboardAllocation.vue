<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { useQuery } from '@tanstack/vue-query'
import { Money } from '@/entities/currency'

import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { formatPct } from '@/shared/lib/format'
import { call } from '@/shared/api/ipc'

import type { DashboardAllocationProps } from './dashboard-allocation.types'
import { AllocationPie } from '@/widgets/allocation-pie'
import { ProgressBar } from '@/shared/ui/progress-bar'

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

</script>

<template>
  <Card size="sm">
    <template #header>
      <p data-widget-title class="font-medium">{{ t(props.by === 'chain' ? 'store.byChain' : 'store.byStore') }}</p>
    </template>
    <SectionState
      :loading="summary.isPending.value"
      :error="summary.error.value"
      :empty="!rows.length"
      :empty-title="t('store.empty')"
      compact
      @retry="summary.refetch()"
    >
      <!-- A widget is placed and sized by the user: the ring only shows when it has a few slices to compare. -->
      <AllocationPie
        v-if="rows.length > 1"
        bare
        :slices="rows.map((row) => ({ label: row.label, value: row.value }))"
        :currency="summary.data.value?.base_currency"
      />
      <div class="flex flex-col gap-1">
        <component
          :is="row.storeId ? RouterLink : 'div'"
          v-for="row in rows"
          :key="row.key"
          :to="row.storeId ? { name: 'wallet', params: { id: row.storeId } } : undefined"
          class="-mx-2 flex flex-col gap-1 rounded-md px-2 py-1"
          :class="row.storeId && 'transition-colors hover:bg-accent'"
        >
          <div class="flex items-center justify-between text-sm">
            <span class="truncate">{{ row.label }}</span>
            <span class="shrink-0 text-muted-foreground">
              <Money :amount="row.value" :currency="summary.data.value?.base_currency" /> · {{ formatPct(row.pct) }}
            </span>
          </div>
          <ProgressBar :value="Number(row.pct ?? 0)" />
        </component>
      </div>
    </SectionState>
  </Card>
</template>
