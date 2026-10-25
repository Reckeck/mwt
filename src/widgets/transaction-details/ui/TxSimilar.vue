<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'

import { Money, MoneyPhrase } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { backQuery } from '@/shared/lib/drill'
import { formatDayMonth } from '@/shared/lib/format'
import type { SumPart } from '@/shared/lib/money-parts'
import { openTxDetails } from '@/shared/lib/tx-details'
import { txFilterToQuery } from '@/shared/lib/tx-filter'
import { txId, txKind, txMeta, txPrincipalLegs } from '@/shared/lib/transactions'
import { useTxContext } from '@/shared/lib/tx-context'

import { pairRate, txPair, txTitle } from '../lib/tx-card'

/** «Похожие · 2» (Paths 1 · 6): two earlier ones like this and a way to all of them. */
const props = defineProps<{ tx: Transaction; items: Transaction[]; filter: TxFilter }>()

const { t } = useI18n()
const route = useRoute()
const context = useTxContext()

const SHOWN = 2

const title = computed(() => txTitle(props.tx, context.categoryName))
const allLabel = computed(() => {
  const kind = txKind(props.tx)
  return kind === 'payment' ? t('txCard.allSimilar.payment', { name: title.value }) : t(`txCard.allSimilar.${kind}`, { pair: title.value })
})
/** The list with this filter, and the chip «← RUB → USDT» back to this card. */
const allTo = computed(() => {
  const query: Record<string, string> = {}
  for (const [key, value] of Object.entries(txFilterToQuery(props.filter))) if (value) query[key] = value
  return { name: 'transactions', query: { ...query, ...backQuery({ to: route.fullPath, label: title.value }) } }
})

const sum = (amount: number, currency: string): SumPart => ({ amount, currency, decimals: 0 })

const rows = computed(() =>
  props.items.slice(0, SHOWN).map((other) => {
    const pair = txPair(other)
    const leg = txPrincipalLegs(other)[0]
    return {
      tx: other,
      date: formatDayMonth(txMeta(other).timestamp),
      what: pair
        ? [sum(pair.out.amount, pair.out.symbol), { text: ' → ' }, sum(pair.into.amount, pair.into.symbol)]
        : [{ text: txMeta(other).comment }],
      // A pair shows its rate, which is no sum to convert.
      value: pair
        ? { amount: pairRate(pair), currency: pair.out.symbol, decimals: 2, convert: false }
        : { amount: Math.abs(Number(leg?.amount ?? 0)), currency: leg?.asset.symbol },
    }
  }),
)
</script>

<template>
  <section v-if="props.items.length" class="grid gap-1" data-tx-similar>
    <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
      <h3 class="text-sm font-semibold">{{ t('txCard.similar', { count: props.items.length }) }}</h3>
      <RouterLink :to="allTo" class="touch-hit-lg text-sm text-primary hover:underline" data-tx-similar-all>{{ allLabel }}</RouterLink>
    </div>
    <Button
      v-for="row in rows"
      :key="txId(row.tx)"
      variant="ghost"
      size="block"
      class="-mx-2 flex items-baseline gap-2 text-sm"
      @click="openTxDetails(row.tx, props.items)"
    >
      <span class="font-medium">{{ row.date }}</span>
      <span class="min-w-0 flex-1 truncate text-muted-foreground tabular-nums">· <MoneyPhrase :parts="row.what" /></span>
      <b class="tabular-nums"><Money v-bind="row.value" /></b>
    </Button>
  </section>
</template>
