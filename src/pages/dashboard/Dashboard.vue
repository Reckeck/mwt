<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { formatMoney, formatPct } from '@/shared/lib/format'
import { call, parseError } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const queryClient = useQueryClient()
const settingsStore = useSettingsStore()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

const refreshBusy = computed(() => summary.isFetching.value)

async function refresh() {
  try {
    const result = await call<RefreshResult>('refresh_all')
    queryClient.setQueryData(['refresh'], result)
    toast.success(t('store.refreshed'), { description: `${result.prices} · ${result.fx}` })
    await queryClient.invalidateQueries({ queryKey: ['summary'] })
  } catch (err) {
    toast.error(t('store.refreshFailed'), { description: parseError(err).message })
  }
}

function pctWidth(value: string | number | null | undefined): string {
  const pct = typeof value === 'string' ? Number(value) : value ?? 0
  return `${Math.min(100, Math.max(0, pct)).toFixed(1)}%`
}

// Dashboard widget definitions matching backend
const widgetDefs = [
  { id: 'total', defaultOrder: 0 },
  { id: 'assets', defaultOrder: 1 },
  { id: 'fx', defaultOrder: 2 },
  { id: 'byChain', defaultOrder: 3 },
  { id: 'byStore', defaultOrder: 4 },
] as const

const visibleWidgets = computed(() => {
  const widgets = settingsStore.vault?.dashboard_widgets ?? []
  return widgetDefs
    .map((def) => {
      const existing = widgets.find((widget) => widget.id === def.id)
      return {
        id: def.id,
        visible: existing?.visible ?? true,
        order: existing?.order ?? def.defaultOrder,
      }
    })
    .filter((widget) => widget.visible)
    .sort((left, right) => left.order - right.order)
})
</script>

<template>
  <div class="grid gap-6 @container">
    <!-- Top row widgets (total, assets, fx) - 3 columns on lg, 2 on sm -->
    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <Card v-if="visibleWidgets.some((widget) => widget.id === 'total')" class="@lg:col-span-1">
        <template #header>
          <p class="text-sm text-muted-foreground">{{ t('store.total') }}</p>
        </template>
        <div>
          <p class="text-2xl font-bold">
            {{ formatMoney(summary.data.value?.total_base, { currency: summary.data.value?.base_currency }) }}
          </p>
          <p class="text-xs text-muted-foreground">
            ≈ {{ formatMoney(summary.data.value?.total_usd, { currency: 'usd' }) }}
          </p>
        </div>
      </Card>

      <Card v-if="visibleWidgets.some((widget) => widget.id === 'assets')" class="@lg:col-span-1">
        <template #header>
          <p class="text-sm text-muted-foreground">Assets</p>
        </template>
        <div class="flex flex-wrap gap-1.5">
          <Badge
            v-for="asset in summary.data.value?.assets ?? []"
            :key="asset.asset.symbol"
            variant="secondary"
          >
            {{ asset.asset.symbol }} · {{ asset.quantity }}
          </Badge>
        </div>
      </Card>

      <Card v-if="visibleWidgets.some((widget) => widget.id === 'fx')" class="@lg:col-span-1">
        <template #header>
          <div class="flex items-center justify-between">
            <p class="text-sm text-muted-foreground">{{ t('store.fxRate') }}</p>
            <Button variant="ghost" size="sm" :disabled="refreshBusy" @click="refresh">
              <RefreshCw class="size-4" :class="refreshBusy && 'animate-spin'" />
              {{ t('store.refresh') }}
            </Button>
          </div>
        </template>
        <div>
          <p class="text-lg font-semibold">
            {{ formatMoney(summary.data.value?.fx_per_usd, { currency: summary.data.value?.base_currency }) }}
          </p>
        </div>
      </Card>
    </div>

    <!-- Bottom row widgets (byChain, byStore) - 2 columns on sm -->
    <div class="grid gap-4 @sm:grid-cols-2">
      <Card v-if="visibleWidgets.some((widget) => widget.id === 'byChain')">
        <template #header>
          <p class="font-medium">{{ t('store.byChain') }}</p>
        </template>
        <div class="space-y-3">
          <div
            v-for="row in summary.data.value?.by_chain ?? []"
            :key="row.label"
            class="flex flex-col gap-1"
          >
            <div class="flex items-center justify-between text-sm">
              <span>{{ row.label }}</span>
              <span class="text-muted-foreground">
                {{ formatMoney(row.value_base, { currency: summary.data.value?.base_currency }) }} · {{ formatPct(row.pct) }}
              </span>
            </div>
            <div class="h-2 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full bg-primary" :style="{ width: pctWidth(row.pct) }" />
            </div>
          </div>
          <p v-if="!summary.data.value?.by_chain.length" class="text-sm text-muted-foreground">
            {{ t('store.empty') }}
          </p>
        </div>
      </Card>

      <Card v-if="visibleWidgets.some((widget) => widget.id === 'byStore')">
        <template #header>
          <p class="font-medium">{{ t('store.byStore') }}</p>
        </template>
        <div class="space-y-3">
          <RouterLink
            v-for="row in summary.data.value?.by_store ?? []"
            :key="row.store_id"
            :to="{ name: 'wallet', params: { id: row.store_id } }"
            class="flex flex-col gap-1 rounded-md px-1 py-0.5 transition-colors hover:bg-accent"
          >
            <div class="flex items-center justify-between text-sm">
              <span class="truncate">{{ row.name }}</span>
              <span class="shrink-0 text-muted-foreground">
                {{ formatMoney(row.value_base, { currency: summary.data.value?.base_currency }) }} · {{ formatPct(row.pct) }}
              </span>
            </div>
            <div class="h-2 overflow-hidden rounded-full bg-muted">
              <div class="h-full rounded-full bg-primary" :style="{ width: pctWidth(row.pct) }" />
            </div>
          </RouterLink>
          <p v-if="!summary.data.value?.by_store.length" class="text-sm text-muted-foreground">
            {{ t('store.empty') }}
          </p>
        </div>
      </Card>
    </div>

    <!-- Empty state when no widgets are visible -->
    <div v-if="!visibleWidgets.length" class="text-center py-12 text-sm text-muted-foreground">
      {{ t('store.empty') }}
    </div>
  </div>
</template>