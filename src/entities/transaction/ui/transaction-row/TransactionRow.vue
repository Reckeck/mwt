<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Money } from '@/entities/currency'

import { ExternalLink } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
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

const props = withDefaults(defineProps<TransactionRowProps>(), { storeId: null, loss: null, openable: false })
const emit = defineEmits<{ open: [tx: Transaction] }>()

/**
 * The row's menu («⋯») comes from the list: entities do not own user actions.
 * Without one, the column holds the explorer link (or stays empty), so every
 * row of a list keeps the same width for its amounts (rule 17).
 */
const slots = defineSlots<{ actions?(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()
const context = useTxContext()

const kind = computed(() => txKind(props.tx))
const meta = computed(() => txMeta(props.tx))
const legs = computed(() => txPrincipalLegs(props.tx))
const internal = computed(() => txIsInternal(props.tx, context.rootOf))
const debt = computed(() => txDebt(props.tx))
const fee = computed(() => txFee(props.tx))
/** Brought in from an exchange; a correction has no categories, so it gets the mark here. */
const imported = computed(() => {
  const origin = meta.value.origin
  return typeof origin === 'object' && 'import' in origin && !meta.value.categories.includes('exchange_import')
})

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

/** Money moved between the user's own wallets is neither earned nor spent: no green or red. */
const ownMove = computed(() => {
  const { from, to } = txSides(props.tx)
  return kind.value === 'transfer' && Boolean(from && to && 'store' in from && 'store' in to)
})

function openExplorer() {
  if (explorer.value) void call('open_explorer', { url: explorer.value })
}

const where = computed(() => context.storeName(shownLegs.value[0]?.store_id ?? ''))
</script>

<template>
  <div class="flex flex-wrap items-start gap-x-3 gap-y-1 py-3">
    <Badge :variant="TX_KIND_BADGE[kind] ?? DEFAULT_TX_BADGE" class="mt-0.5 shrink-0">
      {{ t(`tx.kind.${kind}`) }}
    </Badge>

    <!-- One body for both: an openable row is a button, a plain one a block. -->
    <component
      :is="props.openable ? Button : 'div'"
      v-bind="
        props.openable
          ? { variant: 'ghost', size: 'block', title: t('txDetails.open'), onClick: () => emit('open', props.tx) }
          : {}
      "
      :class="cn('min-w-0 flex-1 basis-56', props.openable && 'block rounded-sm p-0 text-left hover:bg-transparent')"
    >
      <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
        <span class="font-medium">{{ legs.map((leg) => leg.asset.symbol).join(' → ') }}</span>
        <Badge v-for="key in meta.categories" :key="key" variant="outline" class="shrink-0">
          {{ context.categoryName(key) }}
        </Badge>
        <Badge v-if="imported" variant="outline" class="shrink-0">{{ t('tx.imported') }}</Badge>
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
    </component>

    <!-- On a narrow screen the amounts and actions wrap under the text instead of squeezing it. -->
    <div class="ml-auto shrink-0 text-right">
      <p
        v-for="leg in shownLegs"
        :key="leg.store_id + leg.asset.symbol"
        class="whitespace-nowrap font-medium tabular-nums"
        :class="ownMove ? 'text-foreground' : amountClass(leg.amount)"
      >
        <Money :amount="leg.amount" :currency="leg.asset.symbol" signed />
      </p>
      <p v-if="fee" class="whitespace-nowrap text-xs text-muted-foreground tabular-nums">
        {{ t('tx.fee') }} <Money :amount="fee.amount" :currency="fee.asset" />
      </p>
      <p
        v-if="props.loss && Number(props.loss.total) > 0"
        class="whitespace-nowrap text-xs text-warning tabular-nums"
        :title="t('losses.rowHint')"
      >
        {{ t('losses.rowLabel') }} <Money :amount="props.loss.total" :currency="props.loss.currency" />
      </p>
    </div>

    <!-- One column of fixed width in every row: nothing moves on hover, amounts line up. -->
    <div class="flex w-8 shrink-0 justify-end">
      <slot v-if="slots.actions" name="actions" :tx="props.tx" />
      <Button
        v-else-if="explorer"
        variant="ghost"
        size="icon-sm"
        class="text-muted-foreground"
        :title="t('hist.openExplorer')"
        :aria-label="t('hist.openExplorer')"
        @click="openExplorer"
      >
        <ExternalLink class="size-4" />
      </Button>
    </div>
  </div>
</template>
