<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { useQuery } from '@tanstack/vue-query'
import { Money } from '@/entities/currency'

import { assetClassName } from '@/entities/asset-class'
import { Card } from '@/shared/ui/card'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { TOPUP_ADVICE_KEY } from '@/shared/api/queries'
import { SectionState } from '@/shared/ui/section-state'
import { formatPct } from '@/shared/lib/format'
import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

import { ALLOCATION_AXES, type AllocationAxis, type DashboardAllocationProps } from './dashboard-allocation.types'
import { AllocationPie } from '@/widgets/allocation-pie'
import { ProgressBar } from '@/shared/ui/progress-bar'

/**
 * «Распределение» (plan H2: «по сетям» and «по кошелькам» in one): wallets,
 * chains or asset classes against their target shares (plan F).
 */
const props = withDefaults(defineProps<DashboardAllocationProps>(), { by: 'store' })

const { t } = useI18n()
const settings = useSettingsStore()
/** «По сетям» is about crypto: without it the widget offers wallets and classes. */
const axes = computed(() => ALLOCATION_AXES.filter((item) => settings.cryptoEnabled || item !== 'chain'))
const chosenAxis = ref<AllocationAxis>(props.by)
const axis = computed(() => (axes.value.includes(chosenAxis.value) ? chosenAxis.value : 'store'))
function chooseAxis(value: unknown) {
  // A single toggle group hands back `undefined` when the pressed item is clicked again.
  if (typeof value === 'string') chosenAxis.value = value as AllocationAxis
}

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})
/** The classes come with the top-up advice: its rows hold each basket's value and share now. */
const advice = useQuery({
  queryKey: [...TOPUP_ADVICE_KEY, null],
  queryFn: () => call<TopupAdvice>('topup_advice', { amount: null }),
  enabled: computed(() => axis.value === 'class'),
})
const active = computed(() => (axis.value === 'class' ? advice : summary))

interface AllocationRow {
  key: string
  label: string
  value: Amount
  pct: number | null
  /** The basket's target share; classes only. */
  target: Amount | null
  storeId: string | null
}

/** One shape for every axis; only a store row links somewhere. */
const rows = computed<AllocationRow[]>(() => {
  if (axis.value === 'class')
    return (advice.data.value?.rows ?? []).map((row) => ({
      key: row.class.id,
      label: assetClassName(row.class, t),
      value: row.value,
      pct: row.pct_before === null ? null : Number(row.pct_before),
      target: row.class.target_pct,
      storeId: null,
    }))
  const data = summary.data.value
  if (!data) return []
  return axis.value === 'chain'
    ? data.by_chain.map((row) => ({ key: row.label, label: row.label, value: row.value_base, pct: Number(row.pct), target: null, storeId: null }))
    : data.by_store.map((row) => ({
        key: row.store_id,
        label: row.name,
        value: row.value_base,
        pct: Number(row.pct),
        target: null,
        storeId: row.store_id,
      }))
})
const currency = computed(() =>
  axis.value === 'class' ? advice.data.value?.currency : summary.data.value?.base_currency,
)

</script>

<template>
  <Card size="sm">
    <ToggleGroup
      :model-value="axis"
      type="single"
      variant="outline"
      size="sm"
      class="mb-2"
      :aria-label="t('dashboard.allocation.axis')"
      @update:model-value="chooseAxis"
    >
      <ToggleGroupItem v-for="item in axes" :key="item" :value="item" class="px-2 text-xs">
        {{ t(`dashboard.allocation.${item}`) }}
      </ToggleGroupItem>
    </ToggleGroup>
    <SectionState
      :loading="active.isPending.value"
      :error="active.error.value"
      :empty="!rows.length"
      :empty-title="t(axis === 'class' ? 'dashboard.allocation.noClasses' : 'store.empty')"
      compact
      @retry="active.refetch()"
    >
      <!-- A widget is placed and sized by the user: the ring only shows when it has a few slices to compare. -->
      <AllocationPie
        v-if="rows.length > 1"
        bare
        :slices="rows.map((row) => ({ label: row.label, value: row.value }))"
        :currency="currency"
      />
      <div class="flex flex-col gap-1">
        <component
          :is="row.storeId ? RouterLink : 'div'"
          v-for="row in rows"
          :key="row.key"
          :to="row.storeId ? { name: 'wallet', params: { id: row.storeId } } : undefined"
          :data-entity="row.storeId ? `store:${row.storeId}` : undefined"
          class="-mx-2 flex flex-col gap-1 rounded-md px-2 py-1"
          :class="row.storeId && 'transition-colors hover:bg-accent'"
        >
          <div class="flex items-center justify-between text-sm">
            <span class="truncate">{{ row.label }}</span>
            <span class="shrink-0 text-muted-foreground">
              <Money :amount="row.value" :currency="currency" /> · {{ formatPct(row.pct) }}
              <template v-if="row.target !== null"> / {{ formatPct(row.target, { decimals: 0 }) }}</template>
            </span>
          </div>
          <ProgressBar :value="Number(row.pct ?? 0)" />
        </component>
      </div>
    </SectionState>
  </Card>
</template>
