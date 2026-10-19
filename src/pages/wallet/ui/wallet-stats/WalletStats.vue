<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money, PnlText } from '@/entities/currency'
import { useStoreMonths } from '@/entities/store'
import { StatCard } from '@/shared/ui/stat-card'
import { formatDate, formatPct } from '@/shared/lib/format'

/**
 * The wallet at a glance, in the figures its kind is about: a bank or cash
 * shows the month's money in and out, a debt its terms, a coin wallet or an
 * exchange its results; anything else what it holds and how active it is.
 */
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
const store = computed(() => props.overview.store)
const variant = computed(() => {
  const kind = store.value.kind
  if (kind === 'bank' || kind === 'cash') return 'money'
  if (kind === 'liability') return 'debt'
  if (kind === 'crypto' || kind === 'exchange') return 'trading'
  return 'default'
})

const months = useStoreMonths(
  computed(() => store.value.id),
  computed(() => props.rolledUp),
  1,
)
const thisMonth = computed(() => months.data.value?.at(-1))
const total = computed(() => (props.rolledUp ? props.overview.subtree_base : props.overview.own_base))
const unrealized = computed(() => (props.rolledUp ? props.overview.subtree_unrealized_base : props.overview.unrealized_base))
const terms = computed(() => store.value.liability)
const heldSymbols = computed(
  () =>
    props.positions
      .slice(0, 3)
      .map((position) => position.asset.symbol.toUpperCase())
      .join(' · ') || '—',
)
const lastTx = computed(() =>
  props.overview.last_tx_at ? `${t('wallet.lastTx')}: ${formatDate(props.overview.last_tx_at)}` : '—',
)
</script>

<template>
  <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
    <StatCard :label="variant === 'debt' ? t('wallet.stat.debt') : props.rolledUp ? t('wallet.totalWithSub') : t('store.total')">
      <Money :amount="total" :currency="base" :decimals="2" />
      <template v-if="props.rolledUp" #hint>
        {{ t('stores.ownFunds') }}: <Money :amount="props.overview.own_base" :currency="base" :decimals="2" />
      </template>
    </StatCard>

    <template v-if="variant === 'money'">
      <StatCard :label="t('wallet.stat.monthIn')" :loading="months.isPending.value">
        <Money :amount="thisMonth?.inflow ?? '0'" :currency="base" :decimals="2" />
      </StatCard>
      <StatCard :label="t('wallet.stat.monthOut')" :loading="months.isPending.value">
        <Money :amount="thisMonth?.outflow ?? '0'" :currency="base" :decimals="2" />
      </StatCard>
    </template>

    <template v-else-if="variant === 'debt'">
      <StatCard :label="t('wallet.stat.rate')" :value="terms?.rate ? formatPct(terms.rate) : '—'" />
      <StatCard
        :label="t('wallet.stat.monthlyPayment')"
        :hint="terms?.due_day ? t('wallet.stat.dueDay', { day: terms.due_day }) : undefined"
      >
        <Money v-if="terms?.monthly_payment" :amount="terms.monthly_payment" :currency="base" :decimals="2" />
        <template v-else>—</template>
      </StatCard>
    </template>

    <template v-else-if="variant === 'trading'">
      <StatCard :label="t('wallet.stat.unrealized')" :hint="heldSymbols">
        <PnlText :amount="unrealized" :currency="base" :decimals="2" />
      </StatCard>
      <StatCard :label="t('wallet.realizedPnl')" :loading="props.pnlLoading">
        <PnlText :amount="props.pnl?.total_base ?? '0'" :currency="base" :decimals="2" />
        <template v-if="props.pnl" #hint>
          {{ t('wallet.stat.fees') }}: <Money :amount="props.pnl.fees_base" :currency="base" :decimals="2" />
        </template>
      </StatCard>
    </template>

    <template v-else>
      <StatCard :label="t('pos.title')" :value="String(props.positions.length)" :hint="heldSymbols" />
      <StatCard
        :label="t('wallet.realizedPnl')"
        :loading="props.pnlLoading"
        :hint="`${t('wallet.trades')}: ${props.pnl?.legs.length ?? 0}`"
      >
        <PnlText :amount="props.pnl?.total_base ?? '0'" :currency="base" :decimals="2" />
      </StatCard>
    </template>

    <StatCard :label="t('wallet.transactions')" :value="String(props.overview.tx_count)" :hint="lastTx" />
  </div>
</template>
