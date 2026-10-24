<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { CircleAlert, RefreshCw } from 'lucide-vue-next'

import { usePriceRefresh } from '@/features/price-refresh'
import { Money, useFavoriteCurrencies } from '@/entities/currency'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { formatDate, formatPct, signClass } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const route = useRoute()
const settings = useSettingsStore()
const favorites = useFavoriteCurrencies()

const summary = useQuery({ queryKey: ['summary'], queryFn: () => call<PortfolioSummary>('get_summary') })
const table = useQuery({ queryKey: ['fx-table'], queryFn: () => call<FxTable>('get_fx_table') })

const priceRefresh = usePriceRefresh()
const refreshBusy = computed(() => priceRefresh.isPending.value || summary.isFetching.value || table.isFetching.value)
const today = localDate(new Date())

/** No rate for the base currency at all: every total is really in dollars. */
const baseUnpriced = computed(() => summary.data.value?.base_fx_as_of === null)

/** What one unit of USD and of each favourite costs in the base currency. */
const rows = computed(() => {
  const data = table.data.value
  if (!data) return []
  const wanted = [...new Set(['usd', ...favorites.codes.value])].filter(
    (code) => code !== data.base,
  )
  return wanted.flatMap((code) => {
    const row = data.rows.find((entry) => entry.currency === code)
    if (!row) return []
    const day = row.updated_at.slice(0, 10)
    return [
      { code, inBase: row.in_base, change: row.change_pct, stale: day && day < today ? formatDate(row.updated_at) : '' },
    ]
  })
})

const refresh = () => priceRefresh.mutate()
</script>

<template>
  <Card size="sm">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p data-widget-title class="text-sm text-muted-foreground">{{ t('store.fxRate') }}</p>
        <!-- An icon button keeps this header as tall as its neighbours'. -->
        <Button
          variant="ghost"
          size="icon-sm"
          class="-my-2"
          :title="t('store.refresh')"
          :aria-label="t('store.refresh')"
          :disabled="refreshBusy"
          @click="refresh"
        >
          <RefreshCw class="size-4" :class="refreshBusy && 'animate-spin'" />
        </Button>
      </div>
    </template>
    <p v-if="baseUnpriced" class="mb-2 flex items-start gap-1.5 text-xs text-destructive">
      <CircleAlert class="mt-0.5 size-3.5 shrink-0" />
      {{ t('store.fxBaseMissing', { currency: settings.baseCurrency.toUpperCase() }) }}
    </p>
    <SectionState
      :loading="table.isPending.value"
      :error="table.error.value"
      :empty="!rows.length"
      :empty-title="t('store.fxNoFavorites')"
      skeleton="list"
      :rows="2"
      compact
      @retry="table.refetch()"
    >
      <template #empty>
        <RouterLink :to="{ query: { ...route.query, settings: 'accounting', section: 'currencies' } }" :class="buttonVariants({ variant: 'outline', size: 'sm' })">
          {{ t('store.fxPickFavorites') }}
        </RouterLink>
      </template>
      <dl class="grid grid-cols-[auto_1fr_auto_auto] items-baseline gap-x-3 gap-y-1 text-sm">
        <template v-for="row in rows" :key="row.code">
          <dt class="font-medium uppercase">1 {{ row.code }}</dt>
          <dd class="text-right font-semibold"><Money :amount="row.inBase" :convert="false" :decimals="2" /></dd>
          <dd class="min-w-12 text-right text-xs tabular-nums" :class="signClass(row.change)" :title="t('store.fxChange')">
            {{ row.change === null ? '' : formatPct(row.change, { signed: true }) }}
          </dd>
          <dd class="text-right text-xs text-muted-foreground">{{ row.stale }}</dd>
        </template>
      </dl>
    </SectionState>
  </Card>
</template>
