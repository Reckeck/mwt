<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { formatDateTime, formatMoney } from '@/shared/lib/format'
import { DEFAULT_TX_BADGE, TX_KIND_BADGE, txAmountClass, txDetails, txLegs, txMeta, txTag } from '@/shared/lib/transactions'

import type { TransactionRowProps } from './transaction-row.types'

const props = defineProps<TransactionRowProps>()

/** Per-row actions (delete…) come from the page: entities do not own user actions. */
defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()
</script>

<template>
  <div class="flex items-center gap-4 py-3">
    <Badge :variant="TX_KIND_BADGE[txTag(props.tx)] ?? DEFAULT_TX_BADGE">
      {{ t(`tx.kind.${txTag(props.tx)}`) }}
    </Badge>

    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 items-center gap-2">
        <!-- One row per movement, so a trade shows both of its sides
             instead of a single half-truth. -->
        <template v-for="(move, moveIndex) in txLegs(props.tx)" :key="moveIndex">
          <span class="shrink-0 font-medium">
            {{ moveIndex > 0 ? '→ ' : '' }}{{ move.amount }} {{ move.asset.symbol }}
          </span>
          <span class="truncate text-sm text-muted-foreground">{{ move.asset.name }}</span>
        </template>
        <Badge v-for="entry in txMeta(props.tx).categories" :key="entry" variant="outline" class="shrink-0">
          {{ entry }}
        </Badge>
      </div>
      <p class="truncate text-xs text-muted-foreground">
        {{ formatDateTime(txMeta(props.tx).timestamp) }} · {{ props.storeName }}
      </p>
      <!-- The fields that only this kind has: rail, counterparty, pool… -->
      <p v-if="txDetails(props.tx).length" class="truncate text-xs text-muted-foreground">
        {{ txDetails(props.tx).map(([key, value]) => `${t(key)}: ${value}`).join(' · ') }}
      </p>
      <p v-if="txMeta(props.tx).comment" class="truncate text-xs text-muted-foreground">
        {{ txMeta(props.tx).comment }}
      </p>
    </div>

    <div class="shrink-0 text-right">
      <p class="whitespace-nowrap font-medium tabular-nums" :class="txAmountClass(txTag(props.tx))">
        {{ txLegs(props.tx).map((move) => `${move.amount} ${move.asset.symbol}`).join(' → ') }}
      </p>
      <p v-if="txLegs(props.tx)[0]?.price_usd" class="whitespace-nowrap text-xs text-muted-foreground">
        {{ formatMoney(txLegs(props.tx)[0].price_usd ?? '', { currency: 'usd' }) }}
      </p>
    </div>

    <slot name="actions" :tx="props.tx" />
  </div>
</template>