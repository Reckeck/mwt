<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { Copy, CopyPlus, ExternalLink, Pencil, RefreshCw, Trash2 } from 'lucide-vue-next'

import { TransactionCreateDialog } from '@/features/transaction-create'
import { TransactionDelete, useTransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton, TransactionEditDialog } from '@/features/transaction-edit'
import { TransactionFilterBar } from '@/features/transaction-filter'
import { TransactionDetailsSheet } from '@/widgets/transaction-details'
import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import type { UiAction } from '@/shared/ui/context-menu'
import { Dialog } from '@/shared/ui/dialog'
import { call, onEvent } from '@/shared/api/ipc'
import { txId, txStoreId } from '@/shared/lib/transactions'
import { copyText } from '@/shared/lib/clipboard'

import { useTxFilterInUrl } from './tx-filter.state'

const { t } = useI18n()
const queryClient = useQueryClient()

const filter = useTxFilterInUrl()

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

/* --- right-click a row: open, edit, duplicate, copy, delete --- */
const editing = ref<Transaction | null>(null)
const editOpen = computed({
  get: () => editing.value !== null,
  set: (value: boolean) => {
    if (!value) editing.value = null
  },
})
const duplicate = ref<Transaction | null>(null)
const duplicateOpen = computed({
  get: () => duplicate.value !== null,
  set: (value: boolean) => {
    if (!value) duplicate.value = null
  },
})
const deleting = ref<Transaction | null>(null)
const remove = useTransactionDelete()

const copyId = (tx: Transaction) => void copyText(txId(tx))

function rowActions(tx: Transaction): UiAction[] {
  return [
    { id: 'open', label: t('tx.openDetails'), icon: ExternalLink, run: () => openDetails(tx) },
    { id: 'edit', label: t('tx.edit'), icon: Pencil, run: () => (editing.value = tx) },
    { id: 'duplicate', label: t('tx.duplicate'), icon: CopyPlus, run: () => (duplicate.value = tx) },
    { id: 'copy', label: t('tx.copyId'), icon: Copy, separated: true, run: () => void copyId(tx) },
    { id: 'delete', label: t('tx.deleteMenu'), icon: Trash2, destructive: true, separated: true, run: () => (deleting.value = tx) },
  ]
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div data-tour="tx-filter">
      <TransactionFilterBar v-model="filter">
        <template #actions>
          <Button variant="outline" @click="refresh">
            <RefreshCw class="size-4" />
            {{ t('store.refresh') }}
          </Button>
        </template>
      </TransactionFilterBar>
    </div>

    <Card>
      <SectionState
        :loading="txs.isPending.value"
        :error="txs.error.value"
        :empty="!txs.data.value?.length"
        :empty-title="t('filter.empty')"
        :empty-description="t('ui.noDataHint')"
        compact
        @retry="txs.refetch()"
      >
        <div class="-my-3">
          <TransactionsList
            :transactions="txs.data.value ?? []"
            :losses="losses.data.value ?? null"
            openable
            :row-actions="rowActions"
            @open="openDetails"
          >
            <template #actions="{ tx }">
              <TransactionEditButton :tx="tx" />
              <TransactionDelete :id="txId(tx)" />
            </template>
          </TransactionsList>
        </div>
      </SectionState>
    </Card>

    <TransactionDetailsSheet v-model:open="detailsOpen" :tx="opened" />
    <TransactionEditDialog v-if="editing" v-model:open="editOpen" :transaction="editing" />
    <TransactionCreateDialog
      v-if="duplicate"
      v-model:open="duplicateOpen"
      :store-id="txStoreId(duplicate)"
      :prefill="duplicate"
    />
    <!-- Deleting from a menu asks first: the row is gone for good, and a menu has no room
         for the two-step button the row itself uses. -->
    <Dialog
      :open="deleting !== null"
      :title="t('tx.deleteConfirmTitle')"
      :description="t('tx.deleteConfirmHint')"
      size="sm"
      @update:open="(value) => !value && (deleting = null)"
    >
      <template #footer>
        <Button variant="ghost" @click="deleting = null">{{ t('actions.cancel') }}</Button>
        <Button
          variant="destructive"
          :disabled="remove.isPending.value"
          @click="deleting && remove.mutate(txId(deleting), { onSettled: () => (deleting = null) })"
        >
          <Trash2 class="size-4" />
          {{ t('tx.deleteForGood') }}
        </Button>
      </template>
    </Dialog>
  </div>
</template>
