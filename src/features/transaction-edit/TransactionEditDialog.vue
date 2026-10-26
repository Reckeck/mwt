<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { TransactionForm, type TransactionEditDialogProps } from '@/features/transaction-create'
import { AdjustmentDialog } from '@/features/store-adjust'
import { StoreNodeSelect } from '@/entities/store'
import { useStoreTree } from '@/shared/api/queries'
import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { Dialog } from '@/shared/ui/dialog'
import { txKind, txStoreId } from '@/shared/lib/transactions'

const props = defineProps<TransactionEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const tree = useStoreTree()
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
const form = ref<InstanceType<typeof TransactionForm> | null>(null)
const storeId = computed(() => props.storeId ?? txStoreId(props.transaction))
/** A correction is edited on its own screen. */
const isAdjustment = computed(() => txKind(props.transaction) === 'adjustment')
</script>

<template>
  <AdjustmentDialog
    v-if="isAdjustment"
    v-model:open="open"
    :store-id="txStoreId(props.transaction)"
    :transaction="props.transaction"
  />
  <Dialog overlay="tx-edit" v-else v-model:open="open" :title="t('tx.edit')" size="form">
    <div class="@container">
      <!-- Keyed by the transaction: every opening loads it afresh. -->
      <TransactionForm
        v-if="open"
        ref="form"
        :store-id="storeId"
        :transaction="props.transaction"
        compact
        @saved="open = false"
      >
        <!-- One short form, the same as «Новая операция»; the wallet of an existing operation is shown, not changed. -->
        <template #store>
          <FormField for="tx-edit-store" :label="t('quickAdd.store')">
            <StoreNodeSelect id="tx-edit-store" :model-value="storeId" :nodes="nodes" class="w-full" disabled />
          </FormField>
        </template>
      </TransactionForm>
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="transaction-form" type="submit" :disabled="form?.pending">
        {{ t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
