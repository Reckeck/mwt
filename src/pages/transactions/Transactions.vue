<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { RefreshCw } from 'lucide-vue-next'

import { TransactionDelete } from '@/features/transaction-delete'
import { StoreFilter } from '@/features/transaction-filter'
import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { call, onEvent } from '@/shared/api/ipc'
import { txId } from '@/shared/lib/transactions'

const { t } = useI18n()
const queryClient = useQueryClient()

/** Empty string = every store. */
const storeFilter = ref('')

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

// The filter is part of the key, so switching it fetches (and caches) by itself.
const txs = useQuery({
  queryKey: ['transactions', storeFilter],
  queryFn: () => call<Transaction[]>('list_transactions', { storeId: storeFilter.value || null }),
})

/** Both lists, not one `['transactions', 'summary']` key — that prefix matches no query. */
function refresh() {
  void queryClient.invalidateQueries({ queryKey: ['transactions'] })
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
}

void onEvent<RefreshResult>('prices-updated', refresh)

const storeName = (id: string) => stores.data.value?.find((store) => store.id === id)?.name ?? id
</script>

<template>
  <div class="grid gap-6 @container">
    <div class="flex items-center justify-between gap-4">
      <StoreFilter v-model="storeFilter" />

      <Button variant="outline" size="sm" @click="refresh">
        <RefreshCw class="size-4" />
        {{ t('store.refresh') }}
      </Button>
    </div>

    <TransactionsList :transactions="txs.data.value ?? []" :store-name="storeName">
      <template #actions="{ tx }">
        <TransactionDelete :id="txId(tx)" />
      </template>
    </TransactionsList>
  </div>
</template>
