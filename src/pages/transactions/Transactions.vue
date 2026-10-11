<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { Copy, CopyPlus, ExternalLink, Pencil, RefreshCw, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { TransactionCreateDialog } from '@/features/transaction-create'
import { TransactionDelete, useTransactionDelete } from '@/features/transaction-delete'
import { TransactionEditButton, TransactionEditDialog } from '@/features/transaction-edit'
import { TransactionFilterBar } from '@/features/transaction-filter'
import { TransactionDetailsSheet } from '@/widgets/transaction-details'
import { TransactionsList } from '@/entities/transaction'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import type { UiAction } from '@/shared/ui/context-menu'
import { Dialog } from '@/shared/ui/dialog'
import { call, onEvent } from '@/shared/api/ipc'
import { txId, txStoreId } from '@/shared/lib/transactions'

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

async function copyId(tx: Transaction) {
  try {
    await navigator.clipboard.writeText(txId(tx))
    toast.success(t('contacts.copied'))
  } catch {
    toast.error(t('contacts.copyFailed'))
  }
}

function rowActions(tx: Transaction): UiAction[] {
  return [
    { id: 'open', label: t('tx.openDetails'), icon: ExternalLink, run: () => openDetails(tx) },
    { id: 'edit', label: t('tx.edit'), icon: Pencil, run: () => (editing.value = tx) },
    { id: 'duplicate', label: t('tx.duplicate'), icon: CopyPlus, run: () => (duplicate.value = tx) },
    { id: 'copy', label: t('tx.copyId'), icon: Copy, separated: true, run: () => void copyId(tx) },
    { id: 'delete', label: t('tx.delete'), icon: Trash2, destructive: true, separated: true, run: () => (deleting.value = tx) },
  ]
}
</script>

<template>
  <div class="grid gap-4 @container">
    <div class="flex flex-wrap items-end justify-between gap-4">
      <div data-tour="tx-filter"><TransactionFilterBar v-model="filter" /></div>

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
          :row-actions="rowActions"
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
    <TransactionEditDialog v-if="editing" v-model:open="editOpen" :transaction="editing" />
    <TransactionCreateDialog
      v-if="duplicate"
      v-model:open="duplicateOpen"
      :store-id="txStoreId(duplicate)"
      :prefill="duplicate"
    />
    <!-- Deleting from a menu asks first: the row is gone for good. -->
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
          {{ t('tx.delete') }}
        </Button>
      </template>
    </Dialog>
  </div>
</template>
