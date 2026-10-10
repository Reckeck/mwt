<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { TransactionFilterBar } from '@/features/transaction-filter'
import { TransactionDetailsSheet } from '@/widgets/transaction-details'
import { TransactionsList } from '@/entities/transaction'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { txId } from '@/shared/lib/transactions'
import { storeTxFilter } from '@/shared/lib/tx-filter'

import type { StoreTransactionsProps } from './store-transactions.types'

const props = withDefaults(defineProps<StoreTransactionsProps>(), { includeChildren: true })

/** Per-row actions (edit, delete) come from the page. */
defineSlots<{ actions(props: { tx: Transaction }): unknown }>()

const { t } = useI18n()

/** The wallet is fixed by the page; the rest of the filter is the user's. */
const filter = ref<TxFilter>(storeTxFilter(props.storeId, props.includeChildren))
watch(
  () => [props.storeId, props.includeChildren] as const,
  ([storeId, includeChildren]) => {
    filter.value = { ...filter.value, scope: { store_ids: [storeId], include_children: includeChildren } }
  },
)

const txs = useQuery({
  queryKey: computed(() => ['transactions', 'list', filter.value]),
  queryFn: () => call<Transaction[]>('list_transactions', { filter: filter.value }),
  placeholderData: (previous) => previous,
})

const openedId = ref<string | null>(null)
const detailsOpen = ref(false)
const opened = computed(() => (txs.data.value ?? []).find((tx) => txId(tx) === openedId.value) ?? null)
function openDetails(tx: Transaction) {
  openedId.value = txId(tx)
  detailsOpen.value = true
}
</script>

<template>
  <Card>
    <template #header>
      <div class="grid gap-3">
        <p class="font-medium">{{ t('hist.title') }}</p>
        <TransactionFilterBar v-model="filter" fixed-scope id-prefix="store-hist" />
      </div>
    </template>
    <p v-if="txs.isSuccess.value && !txs.data.value?.length" class="text-sm text-muted-foreground">
      {{ t('filter.empty') }}
    </p>
    <div v-else class="-my-3">
      <TransactionsList :transactions="txs.data.value ?? []" :store-id="props.storeId" openable @open="openDetails">
        <template #actions="{ tx }">
          <slot name="actions" :tx="tx" />
        </template>
      </TransactionsList>
    </div>
    <TransactionDetailsSheet v-model:open="detailsOpen" :tx="opened" />
  </Card>
</template>
