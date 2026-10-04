<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import Badge from '@/components/ui/badge.vue'
import Button from '@/components/ui/button.vue'
import Card from '@/components/ui/card.vue'
import { formatDateTime, formatMoney } from '@/lib/format'
import { call, onEvent } from '@/lib/ipc'

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

void onEvent<RefreshResult>('prices-updated', () => {
  void queryClient.invalidateQueries({ queryKey: ['transactions', 'summary'] })
})

const kindBadge: Record<TxKind, 'default' | 'secondary' | 'destructive' | 'outline'> = {
  income: 'default',
  expense: 'destructive',
  transfer: 'secondary',
  trade: 'secondary',
  fee: 'outline',
  paper_trade: 'outline',
}

const storeName = (id: string) => stores.data.value?.find((tx) => tx.id === id)?.name ?? id

async function remove(id: string) {
  await call('delete_transaction', { id })
  await queryClient.invalidateQueries({ queryKey: ['transactions', 'summary'] })
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

      <Button size="sm" @click="() => queryClient.invalidateQueries({ queryKey: ['transactions', 'summary'] })">
        {{ t('store.refresh') }}
      </Button>
    </div>

    <Card>
      <template #header>
        <p class="font-medium">{{ t('tx.title') }}</p>
      </template>

      <div class="divide-y divide-border">
        <div
          v-for="tx in txs.data.value ?? []"
          :key="tx.id"
          class="flex items-center gap-4 py-3"
        >
          <Badge :variant="kindBadge[tx.kind] ?? 'secondary'">
            {{ t(`tx.kind.${tx.kind}`) }}
          </Badge>

          <div class="min-w-0 flex-1">
            <div class="flex min-w-0 items-center gap-2">
              <span class="shrink-0 font-medium">{{ tx.asset.symbol }}</span>
              <span class="truncate text-sm text-muted-foreground">{{ tx.asset.name }}</span>
              <Badge v-if="tx.category" variant="outline" class="shrink-0">{{ tx.category }}</Badge>
            </div>
            <p class="truncate text-xs text-muted-foreground">
              {{ formatDateTime(tx.timestamp) }} · {{ storeName(tx.store_id) }}
            </p>
            <p v-if="tx.comment" class="truncate text-xs text-muted-foreground">{{ tx.comment }}</p>
          </div>

          <div class="shrink-0 text-right">
            <p
              class="whitespace-nowrap font-medium tabular-nums"
              :class="(tx.kind === 'income' ? 'text-emerald-600' : tx.kind === 'expense' ? 'text-destructive' : '')"
            >
              {{ tx.amount }} {{ tx.asset.symbol }}
            </p>
            <p v-if="tx.price_usd" class="whitespace-nowrap text-xs text-muted-foreground">
              {{ formatMoney(tx.price_usd, { currency: 'usd' }) }}
            </p>
          </div>

          <Button variant="ghost" size="icon" class="shrink-0" :title="t('tx.delete')" @click="remove(tx.id)">
            <span class="sr-only">{{ t('tx.delete') }}</span>
            <span class="text-destructive">✕</span>
          </Button>
        </div>

        <p v-if="!txs.data.value?.length" class="py-6 text-center text-sm text-muted-foreground">
          {{ t('tx.empty') }}
        </p>
      </div>
    </Card>
  </div>
</template>