<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ExternalLink } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import {
  DEFAULT_TX_BADGE,
  TX_KIND_BADGE,
  amountClass,
  txDebt,
  txDetails,
  txExplorerUrl,
  txFee,
  txHasNoPrice,
  txIsInternal,
  txKind,
  txLegForStore,
  txMeta,
  txPrincipalLegs,
  txSides,
} from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import type { TransactionRowProps } from './transaction-row.types'

const props = withDefaults(defineProps<TransactionRowProps>(), { storeId: null })

/** Per-row actions (edit, delete…) come from the page: entities do not own user actions. */
defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()
const context = useTxContext()

const kind = computed(() => txKind(props.tx))
const meta = computed(() => txMeta(props.tx))
const legs = computed(() => txPrincipalLegs(props.tx))
const internal = computed(() => txIsInternal(props.tx, context.rootOf))
const debt = computed(() => txDebt(props.tx))
const fee = computed(() => txFee(props.tx))

/** Where the money went or came from, seen from this list's wallet. */
const route = computed(() => {
  const { from, to } = txSides(props.tx)
  if (from && to) return `${context.sideLabel(from)} → ${context.sideLabel(to)}`
  if (from) return `← ${context.sideLabel(from)}`
  if (to) return `→ ${context.sideLabel(to)}`
  return ''
})

/** In a wallet's history only its own side; in the full list every principal leg. */
const shownLegs = computed(() =>
  props.storeId ? [txLegForStore(props.tx, props.storeId)].filter(Boolean) : legs.value,
)
const explorer = computed(() => txExplorerUrl(props.tx))

function openExplorer() {
  if (explorer.value) void call('open_explorer', { url: explorer.value })
}

const where = computed(() => context.storeName(shownLegs.value[0]?.store_id ?? ''))
</script>

<template>
  <div class="flex items-start gap-3 py-3">
    <Badge :variant="TX_KIND_BADGE[kind] ?? DEFAULT_TX_BADGE" class="mt-0.5 shrink-0">
      {{ t(`tx.kind.${kind}`) }}
    </Badge>

    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        <span class="font-medium">{{ legs.map((leg) => leg.asset.symbol).join(' → ') }}</span>
        <Badge v-for="key in meta.categories" :key="key" variant="outline" class="shrink-0">
          {{ context.categoryName(key) }}
        </Badge>
        <Badge v-if="internal" variant="secondary" class="shrink-0">{{ t('tx.internal') }}</Badge>
        <Badge v-if="debt" variant="secondary" class="shrink-0">{{ t(`debt.${debt}`) }}</Badge>
        <Badge v-if="txHasNoPrice(props.tx)" variant="outline" class="shrink-0 text-muted-foreground">
          {{ t('tx.noPrice') }}
        </Badge>
      </div>
      <p class="truncate text-xs text-muted-foreground">
        {{ formatDateTime(meta.timestamp) }}
        <!-- A two-sided route already names the wallet. -->
        <template v-if="!route.includes('→') || route.startsWith('→')"> · {{ where }}</template>
        <template v-if="route"> · {{ route }}</template>
        <template v-if="meta.contact_id"> · {{ context.contactName(meta.contact_id) }}</template>
      </p>
      <p v-if="txDetails(props.tx).length" class="truncate text-xs text-muted-foreground">
        {{ txDetails(props.tx).map(([key, value]) => `${t(key)}: ${value}`).join(' · ') }}
      </p>
      <p v-if="meta.comment" class="truncate text-xs text-muted-foreground">{{ meta.comment }}</p>
    </div>

    <div class="shrink-0 text-right">
      <p
        v-for="leg in shownLegs"
        :key="leg.store_id + leg.asset.symbol"
        class="whitespace-nowrap font-medium tabular-nums"
        :class="amountClass(leg.amount)"
      >
        {{ leg.amount }} {{ leg.asset.symbol }}
      </p>
      <p v-if="fee" class="whitespace-nowrap text-xs text-muted-foreground tabular-nums">
        {{ t('tx.fee') }} {{ fee.amount }} {{ fee.asset }}
      </p>
    </div>

    <Button
      v-if="explorer"
      variant="ghost"
      size="icon"
      class="size-8 shrink-0"
      :title="t('hist.openExplorer')"
      :aria-label="t('hist.openExplorer')"
      @click="openExplorer"
    >
      <ExternalLink class="size-4" />
    </Button>
    <slot name="actions" :tx="props.tx" />
  </div>
</template>
