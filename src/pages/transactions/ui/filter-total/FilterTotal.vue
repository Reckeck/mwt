<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { groupByDay } from '@/entities/transaction'
import { formatMoney, pluralForm } from '@/shared/lib/format'
import { useTxContext } from '@/shared/lib/tx-context'

/**
 * «3 оплаты · 36 $» (Paths 1 · 5): how many operations the filter keeps and
 * their net in the base currency, once every page of them is loaded.
 */
const props = defineProps<{
  rows: Transaction[]
  total: number
  kinds: TxKind[]
  fxPerUsd?: string | number
  baseCurrency?: string
}>()

const { t, locale } = useI18n()
const context = useTxContext()

const payments = computed(() => props.kinds.length === 1 && props.kinds[0] === 'payment')
const count = computed(() => {
  const noun = payments.value ? 'payments' : 'ops'
  return t(`tx.filterTotal.${noun}.${pluralForm(props.total, locale.value)}`, { count: props.total })
})

const sum = computed(() => {
  if (!props.baseCurrency || props.rows.length < props.total) return null
  const days = groupByDay(props.rows, Number(props.fxPerUsd ?? 1), context.rootOf)
  const net = days.reduce((total, day) => total + day.totalBase, 0)
  // Spending alone reads as what was spent («36 $»), a mix keeps its sign.
  const shown = payments.value ? Math.abs(net) : net
  return formatMoney(shown, { currency: props.baseCurrency, signed: !payments.value })
})
</script>

<template>
  <span class="text-sm whitespace-nowrap text-muted-foreground tabular-nums" data-filter-total>
    {{ count }}<template v-if="sum"> · <b class="font-semibold text-foreground">{{ sum }}</b></template>
  </span>
</template>
