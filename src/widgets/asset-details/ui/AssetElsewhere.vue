<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { assetKey } from '@/shared/lib/asset-ref'
import { CHAIN_LABELS } from '@/shared/lib/chains'
import { formatQuantity } from '@/shared/lib/format'

/** The same ticker on every wallet, each with its network: a bare ticker matches any. */
const props = defineProps<{ asset: AssetRef; storeId: string; decimals: number }>()
const { t } = useI18n()

const holdings = useQuery({
  // no placeholder: another asset's holdings
  queryKey: computed(() => ['asset-holdings', assetKey(props.asset)]),
  queryFn: () => call<AssetHoldings>('get_asset_holdings', { asset: props.asset }),
})
const rows = computed(() => holdings.data.value?.rows ?? [])
const network = (row: AssetHolding) => (row.asset.chain && row.asset.chain !== 'fiat' ? CHAIN_LABELS[row.asset.chain] : '')
</script>

<template>
  <section class="grid gap-2">
    <h3 class="heading-card">{{ t('assetDetails.elsewhere') }}</h3>
    <SectionState
      :loading="holdings.isPending.value"
      :error="holdings.error.value"
      :empty="!rows.length"
      :empty-title="t('pos.empty')"
      skeleton="list"
      :rows="2"
      compact
      @retry="holdings.refetch()"
    >
      <ul class="grid gap-1 text-sm">
        <li
          v-for="row in rows"
          :key="`${row.store_id}-${assetKey(row.asset)}`"
          class="flex flex-wrap items-baseline gap-x-3 gap-y-0.5 rounded-md px-2 py-1.5"
          :class="row.store_id === props.storeId && 'bg-accent/50'"
        >
          <span class="min-w-0 flex-1 truncate">
            <RouterLink :to="{ name: 'wallet', params: { id: row.store_id } }" :data-entity="`store:${row.store_id}`" class="font-medium hover:underline">
              {{ [...row.path, row.store_name].join(' › ') }}
            </RouterLink>
            <span v-if="network(row)" class="ml-2 text-xs text-muted-foreground">{{ network(row) }}</span>
            <Badge v-if="row.archived" variant="outline" class="ml-2">{{ t('stores.archived') }}</Badge>
          </span>
          <span class="tabular-nums text-muted-foreground">{{ formatQuantity(row.quantity, props.decimals) }}</span>
          <span class="text-right">
            <Money :amount="row.value_base" :currency="holdings.data.value?.base_currency" :decimals="2" />
            <span v-if="row.peg_valued" class="ml-1 text-xs text-muted-foreground">{{ t('assetDetails.pegValued') }}</span>
          </span>
        </li>
      </ul>
      <p v-if="rows.length > 1" class="mt-2 flex justify-between border-t border-border px-2 pt-2 text-sm font-medium">
        <span>{{ t('assetDetails.total') }} · {{ formatQuantity(holdings.data.value?.total_quantity ?? '0', props.decimals) }}</span>
        <Money :amount="holdings.data.value?.total_value_base" :currency="holdings.data.value?.base_currency" :decimals="2" />
      </p>
    </SectionState>
  </section>
</template>
