<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { TransactionCreateDialog } from '@/features/transaction-create'
import { useTransactionDelete } from '@/features/transaction-delete'
import { TransactionEditDialog } from '@/features/transaction-edit'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { txId, txStoreId } from '@/shared/lib/transactions'

import type { TxRowMenu } from './transaction-row-menu.types'

const props = defineProps<{ menu: TxRowMenu }>()

const { t } = useI18n()
const remove = useTransactionDelete()

const editOpen = computed({
  get: () => props.menu.editing.value !== null,
  set: (value: boolean) => {
    if (!value) props.menu.editing.value = null
  },
})
const duplicateOpen = computed({
  get: () => props.menu.duplicating.value !== null,
  set: (value: boolean) => {
    if (!value) props.menu.duplicating.value = null
  },
})

function confirmDelete() {
  const tx = props.menu.deleting.value
  if (!tx) return
  remove.mutate(txId(tx), {
    onSettled: () => {
      props.menu.deleting.value = null
    },
  })
}
</script>

<template>
  <TransactionEditDialog
    v-if="props.menu.editing.value"
    v-model:open="editOpen"
    :transaction="props.menu.editing.value"
    :store-id="props.menu.storeId.value ?? undefined"
  />
  <TransactionCreateDialog
    v-if="props.menu.duplicating.value"
    v-model:open="duplicateOpen"
    :store-id="txStoreId(props.menu.duplicating.value)"
    :prefill="props.menu.duplicating.value"
  />
  <!-- Deleting from a menu asks first: the row is gone for good. -->
  <Dialog
    :open="props.menu.deleting.value !== null"
    :title="t('tx.deleteConfirmTitle')"
    :description="t('tx.deleteConfirmHint')"
    size="sm"
    @update:open="(value) => !value && (props.menu.deleting.value = null)"
  >
    <template #footer>
      <Button variant="ghost" @click="props.menu.deleting.value = null">{{ t('actions.cancel') }}</Button>
      <Button variant="destructive" :disabled="remove.isPending.value" @click="confirmDelete">
        <Trash2 class="size-4" />
        {{ t('tx.deleteForGood') }}
      </Button>
    </template>
  </Dialog>
</template>
