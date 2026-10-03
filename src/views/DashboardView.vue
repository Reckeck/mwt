<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'

import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import { formatMoney, formatPct } from '@/lib/format'
import { call } from '@/lib/ipc'

const { t } = useI18n()
const queryClient = useQueryClient()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})

const refreshBusy = computed(() => summary.isFetching.value)

async function refresh() {
  const result = await call<RefreshResult>('refresh_all')
  queryClient.setQueryData(['refresh'], result)
  await queryClient.invalidateQueries({ queryKey: ['summary'] })
}

function pctWidth(v: string | number | null | undefined): string {
  const n = typeof v === 'string' ? Number(v) : v ?? 0
  return `${Math.min(100, Math.max(0, n)).toFixed(1)}%`
}
</script>

<template>
  <div class="grid gap-6">
    <div class="grid gap-4 @sm:grid-cols-2 @lg:grid-cols-3">
      <Card>
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

      <Card>
        <template #header>
          <p class="text-sm text-muted-foreground">Assets</p>
        </template>
        <div class="flex flex-wrap gap-1.5">
          <Badge v-for="a in summary.data.value?.assets ?? []" :key="a.asset.symbol" variant="secondary">
            {{ a.asset.symbol }} · {{ a.quantity }}
          </Badge>
        </div>
      </Card>

      <Card>
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

    <div class="grid gap-4 @sm:grid-cols-2">
      <Card>
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

      <Card>
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
  </div>
</template>