<script setup lang="ts">
import { computed } from 'vue'

import { Money } from '@/entities/currency'
import { groupByDay } from '@/entities/transaction'
import { useTxContext } from '@/shared/lib/tx-context'

import { FilterCount } from '../filter-count'

/**
 * `3 payments · 36 $` (Paths 1 · 5): how many operations the filter keeps and
 * their net in the base currency, once every page of them is loaded.
 */
const props = defineProps<{
  filter: TxFilter
  rows: Transaction[]
  total: number
  kinds: TxKind[]
  fxPerUsd?: string | number
  baseCurrency?: string
}>()

const context = useTxContext()

const payments = computed(() => props.kinds.length === 1 && props.kinds[0] === 'payment')
const sum = computed(() => {
  if (!props.baseCurrency || props.rows.length < props.total) return null
  const days = groupByDay(props.rows, Number(props.fxPerUsd ?? 1), context.rootOf)
  const net = days.reduce((total, day) => total + day.totalBase, 0)
  // Spending alone reads as what was spent («36 $»), a mix keeps its sign.
  const shown = payments.value ? Math.abs(net) : net
  return { amount: shown, currency: props.baseCurrency }
})
</script>

<template>
  <FilterCount :filter="filter" :total="total" :payments="payments" data-filter-total>
    <template v-if="sum"> · <b class="font-semibold text-foreground"><Money :amount="sum.amount" :currency="sum.currency" :signed="!payments" /></b></template>
  </FilterCount>
</template>
