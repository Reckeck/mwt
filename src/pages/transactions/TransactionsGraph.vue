<script setup lang="ts">
import { computed, ref } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { TransactionFilterBar } from '@/features/transaction-filter'
import { TransactionDetailsSheet } from '@/widgets/transaction-details'
import { TxGraphView } from '@/widgets/tx-graph'
import { call } from '@/shared/api/ipc'
import { txId } from '@/shared/lib/transactions'

import { sharedTxFilter } from './tx-filter.state'

const filter = sharedTxFilter

// Same key as the list tab: the details panel reads the operation from there.
const txs = useQuery({
  queryKey: computed(() => ['transactions', 'list', filter.value]),
  queryFn: () => call<Transaction[]>('list_transactions', { filter: filter.value }),
})

const openedId = ref<string | null>(null)
const detailsOpen = ref(false)
const opened = computed(() => (txs.data.value ?? []).find((tx) => txId(tx) === openedId.value) ?? null)
function openDetails(id: string) {
  openedId.value = id
  detailsOpen.value = true
}
</script>

<template>
  <div class="grid gap-4">
    <TransactionFilterBar v-model="filter" id-prefix="graph-filter" />
    <TxGraphView :filter="filter" @open="openDetails" />
    <TransactionDetailsSheet v-model:open="detailsOpen" :tx="opened" />
  </div>
</template>
