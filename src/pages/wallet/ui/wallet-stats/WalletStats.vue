<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money, PnlText } from '@/entities/currency'
import { StatCard } from '@/shared/ui/stat-card'
import { formatDate } from '@/shared/lib/format'

/** The wallet at a glance: its value, what it holds, realized P&L, and how active it is. */
const props = defineProps<{
  overview: StoreOverview
  pnl: RealizedPnl | undefined
  pnlLoading: boolean
  positions: StorePosition[]
  /** Totals include the sub-accounts. */
  rolledUp: boolean
}>()

const { t } = useI18n()
const base = computed(() => props.overview.base_currency)
const heldSymbols = computed(
  () =>
    props.positions
      .slice(0, 3)
      .map((position) => position.asset.symbol.toUpperCase())
      .join(' · ') || '—',
)
</script>

<template>
  <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
    <StatCard :label="props.rolledUp ? t('wallet.totalWithSub') : t('store.total')">
      <Money :amount="props.rolledUp ? props.overview.subtree_base : props.overview.own_base" :currency="base" :decimals="2" />
      <template v-if="props.rolledUp" #hint>
        {{ t('stores.ownFunds') }}: <Money :amount="props.overview.own_base" :currency="base" :decimals="2" />
      </template>
    </StatCard>
    <StatCard :label="t('pos.title')" :value="String(props.positions.length)" :hint="heldSymbols" />
    <StatCard
      :label="t('wallet.realizedPnl')"
      :loading="props.pnlLoading"
      :hint="`${t('wallet.trades')}: ${props.pnl?.legs.length ?? 0}`"
    >
      <PnlText :amount="props.pnl?.total_base ?? '0'" :currency="base" :decimals="2" />
    </StatCard>
    <StatCard
      :label="t('wallet.transactions')"
      :value="String(props.overview.tx_count)"
      :hint="props.overview.last_tx_at ? `${t('wallet.lastTx')}: ${formatDate(props.overview.last_tx_at)}` : '—'"
    />
  </div>
</template>
