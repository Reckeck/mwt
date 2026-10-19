<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { formatQuantity } from '@/shared/lib/format'

/** Rows shown before «ещё N»: a widget is a glance, the wallets have the full list. */
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
</script>

<template>
  <Card size="sm">
    <template #header>
      <p data-widget-title class="text-sm text-muted-foreground">{{ t('store.assets') }}</p>
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
          <span class="w-16 shrink-0 truncate font-medium">{{ asset.asset.symbol.toUpperCase() }}</span>
          <span class="min-w-0 flex-1 truncate text-xs tabular-nums text-muted-foreground">
            {{ formatQuantity(asset.quantity) }}
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
