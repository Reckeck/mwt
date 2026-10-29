<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money, PnlText } from '@/entities/currency'
import { EntityLink } from '@/shared/ui/entity-link'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { formatPct, formatQuantity, signClass } from '@/shared/lib/format'

/** Rows shown before `N more`: a widget is a glance, the wallets have the full list. */
const SHOWN = 8

const { t } = useI18n()

const summary = useQuery({
  queryKey: ['summary'],
  queryFn: () => call<PortfolioSummary>('get_summary'),
})
/** The biggest holdings first. */
const assets = computed(() =>
  [...(summary.data.value?.assets ?? [])].sort((left, right) => Number(right.value_base) - Number(left.value_base)),
)
const expanded = ref(false)
const shown = computed(() => (expanded.value ? assets.value : assets.value.slice(0, SHOWN)))

/** What the holdings gained or lost over 24 hours, from each price's own change (plan H3). */
const dayChange = computed(() => {
  const priced = assets.value.filter((asset) => asset.change_24h_pct !== null)
  if (!priced.length) return null
  const delta = priced.reduce((total, asset) => {
    const pct = Number(asset.change_24h_pct)
    return total + (Number(asset.value_base) * pct) / (100 + pct)
  }, 0)
  const before = priced.reduce((total, asset) => total + Number(asset.value_base), 0) - delta
  return { delta, pct: before > 0 ? (delta / before) * 100 : null }
})
</script>

<template>
  <Card size="sm">
    <template #header>
      <div class="flex items-baseline justify-between gap-2">
        <p data-widget-title class="text-sm text-muted-foreground">{{ t('store.assets') }}</p>
        <span v-if="dayChange" class="text-xs" :title="t('assets.change24h')">
          <PnlText :amount="dayChange.delta" :pct="dayChange.pct" :currency="summary.data.value?.base_currency" :decimals="0" />
        </span>
      </div>
    </template>
    <SectionState
      :loading="summary.isPending.value"
      :error="summary.error.value"
      :empty="!assets.length"
      :empty-title="t('store.empty')"
      skeleton="list"
      :rows="3"
      compact
      @retry="summary.refetch()"
    >
      <ul class="grid gap-1 text-sm">
        <li v-for="asset in shown" :key="assetKey(asset.asset)" class="flex items-baseline gap-2">
          <EntityLink
            :target="{ kind: 'asset', id: assetKey(asset.asset) }"
            :label="asset.asset.symbol.toUpperCase()"
            plain
            class="w-16 shrink-0 truncate font-medium"
          />
          <span class="min-w-0 flex-1 truncate text-xs tabular-nums text-muted-foreground">
            {{ formatQuantity(asset.quantity) }}
          </span>
          <span class="w-14 shrink-0 text-right text-xs tabular-nums" :class="signClass(asset.change_24h_pct)">
            {{ asset.change_24h_pct === null ? '' : formatPct(asset.change_24h_pct, { signed: true }) }}
          </span>
          <Money class="shrink-0" :amount="asset.value_base" :currency="summary.data.value?.base_currency" :decimals="2" />
        </li>
      </ul>
      <Button
        v-if="assets.length > SHOWN"
        variant="link"
        size="sm"
        class="mt-1 h-auto w-fit px-0"
        @click="expanded = !expanded"
      >
        {{ expanded ? t('ui.showLess') : t('assets.more', { count: assets.length - SHOWN }) }}
      </Button>
    </SectionState>
  </Card>
</template>
