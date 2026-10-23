<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import { formatDateTime } from '@/shared/lib/format'
import { DEFAULT_TX_BADGE, TX_KIND_BADGE, txKind, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import { txPair, txTitle } from '../lib/tx-card'

/** «RUB → USDT · операция · ‹ › соседние», the kind, the sums large, the moment (Paths 1 · 6). */
const props = defineProps<{ tx: Transaction; prev: boolean; next: boolean; neighbours: boolean }>()
const emit = defineEmits<{ go: [step: -1 | 1] }>()

const { t } = useI18n()
const context = useTxContext()

const kind = computed(() => txKind(props.tx))
const title = computed(() => txTitle(props.tx, context.categoryName))
/** An exchange reads «−45 000 ₽ → +480 USDT»; anything else lists its legs. */
const sums = computed(() => {
  const pair = txPair(props.tx)
  const legs = txPrincipalLegs(props.tx)
  if (!pair) return legs.map((leg) => ({ amount: leg.amount, symbol: leg.asset.symbol }))
  return [
    { amount: String(-pair.out.amount), symbol: pair.out.symbol },
    { amount: String(pair.into.amount), symbol: pair.into.symbol },
  ]
})
</script>

<template>
  <SheetHeader class="gap-0.5 pe-12">
    <SheetTitle class="text-lg">{{ title }}</SheetTitle>
    <SheetDescription class="flex items-center gap-1 text-xs">
      {{ t('txCard.subtitle') }}
      <template v-if="props.neighbours">
        ·
        <Button variant="ghost" size="icon-xs" :disabled="!props.prev" :aria-label="t('txCard.prev')" data-tx-prev @click="emit('go', -1)">
          <ChevronLeft class="size-3.5" />
        </Button>
        <Button variant="ghost" size="icon-xs" :disabled="!props.next" :aria-label="t('txCard.next')" data-tx-next @click="emit('go', 1)">
          <ChevronRight class="size-3.5" />
        </Button>
        {{ t('txCard.neighbours') }}
      </template>
    </SheetDescription>
  </SheetHeader>

  <div class="grid justify-items-start gap-1.5 px-4">
    <Badge :variant="TX_KIND_BADGE[kind] ?? DEFAULT_TX_BADGE">{{ t(`tx.kindNoun.${kind}`) }}</Badge>
    <p class="flex flex-wrap items-baseline gap-x-2 text-xl font-semibold tabular-nums" data-tx-sums>
      <template v-for="(sum, index) in sums" :key="sum.symbol + index">
        <span v-if="index" class="text-muted-foreground" aria-hidden="true">→</span>
        <Money :amount="sum.amount" :currency="sum.symbol" signed />
      </template>
    </p>
    <p class="text-xs text-muted-foreground">{{ formatDateTime(txMeta(props.tx).timestamp) }}</p>
  </div>
</template>
