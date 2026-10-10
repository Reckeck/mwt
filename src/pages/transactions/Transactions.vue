<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'

import { TransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton } from '@/features/transaction-edit'
import { TransactionFilterBar } from '@/features/transaction-filter'
import { TransactionDetailsSheet } from '@/widgets/transaction-details'
import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call, onEvent } from '@/shared/api/ipc'
import { txId } from '@/shared/lib/transactions'

import { sharedTxFilter } from './tx-filter.state'

const { t } = useI18n()
const queryClient = useQueryClient()

const filter = sharedTxFilter

// The filter is part of the key, so changing it fetches (and caches) by itself.
const txs = useQuery({
  queryKey: computed(() => ['transactions', 'list', filter.value]),
  queryFn: () => call<Transaction[]>('list_transactions', { filter: filter.value }),
  placeholderData: (previous) => previous,
})

const losses = useQuery({
  queryKey: ['transactions', 'losses'],
  queryFn: () => call<Record<string, TxLoss>>('get_tx_losses'),
})

/** The details panel shows the latest copy of the opened transaction. */
const openedId = ref<string | null>(null)
const detailsOpen = ref(false)
const opened = computed(() => (txs.data.value ?? []).find((tx) => txId(tx) === openedId.value) ?? null)
function openDetails(tx: Transaction) {
  openedId.value = txId(tx)
  detailsOpen.value = true
}

/** Both lists, not one `['transactions', 'summary']` key — that prefix matches no query. */
function refresh() {
  void queryClient.invalidateQueries({ queryKey: ['transactions'] })
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
}

void onEvent<RefreshResult>('prices-updated', refresh)
</script>

<template>
  <div class="grid gap-6 @container">
    <div class="flex flex-wrap items-end justify-between gap-4">
      <TransactionFilterBar v-model="filter" />

      <Button variant="outline" size="sm" class="h-9" @click="refresh">
        <RefreshCw class="size-4" />
        {{ t('store.refresh') }}
      </Button>
    </div>

    <Card>
      <p v-if="txs.isSuccess.value && !txs.data.value?.length" class="text-sm text-muted-foreground">
        {{ t('filter.empty') }}
      </p>
      <div v-else class="-my-3">
        <TransactionsList
          :transactions="txs.data.value ?? []"
          :losses="losses.data.value ?? null"
          openable
          @open="openDetails"
        >
          <template #actions="{ tx }">
            <TransactionEditButton :tx="tx" />
            <TransactionDelete :id="txId(tx)" />
          </template>
        </TransactionsList>
      </div>
    </Card>

    <TransactionDetailsSheet v-model:open="detailsOpen" :tx="opened" />
  </div>
</template>
