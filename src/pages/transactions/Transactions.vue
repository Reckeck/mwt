<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { call, onEvent, parseError } from '@/shared/api/ipc'

const { t } = useI18n()
const queryClient = useQueryClient()

const storeFilter = ref('')

const stores = useQuery({
  queryKey: ['stores'],
  queryFn: () => call<Store[]>('list_stores'),
})

const txs = useQuery({
  queryKey: ['transactions'],
  queryFn: () => call<Transaction[]>('list_transactions', { storeId: storeFilter.value || null }),
})

/** Both lists, not one `['transactions', 'summary']` key — that prefix matches no query. */
function refresh() {
  void queryClient.invalidateQueries({ queryKey: ['transactions'] })
  void queryClient.invalidateQueries({ queryKey: ['summary'] })
}

void onEvent<RefreshResult>('prices-updated', refresh)

const storeName = (id: string) => stores.data.value?.find((tx) => tx.id === id)?.name ?? id

async function remove(id: string) {
  try {
    await call('delete_transaction', { id })
    toast.success(t('tx.deleted'))
    refresh()
  } catch (err) {
    toast.error(t('tx.deleteFailed'), { description: parseError(err).message })
  }
}
</script>

<template>
  <div class="grid gap-6 @container">
    <div class="flex items-center justify-between gap-4">
      <select
        v-model="storeFilter"
        class="h-9 rounded-md border border-input bg-transparent px-3 text-sm"
        @change="() => queryClient.invalidateQueries({ queryKey: ['transactions'] })"
      >
        <option value="">{{ t('tx.type') }} · все</option>
        <option v-for="store in stores.data.value ?? []" :key="store.id" :value="store.id">{{ store.name }}</option>
      </select>

      <Button size="sm" @click="refresh">
        {{ t('store.refresh') }}
      </Button>
    </div>

    <TransactionsList :transactions="txs.data.value ?? []" :store-name="storeName" @remove="remove" />
  </div>
</template>