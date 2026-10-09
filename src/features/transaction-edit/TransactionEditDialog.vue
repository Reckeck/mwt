<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { TransactionForm, type TransactionEditDialogProps } from '@/features/transaction-create'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { txStoreId } from '@/shared/lib/transactions'

const props = defineProps<TransactionEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const form = ref<InstanceType<typeof TransactionForm> | null>(null)
const storeId = computed(() => props.storeId ?? txStoreId(props.transaction))
</script>

<template>
  <Dialog v-model:open="open" :title="t('tx.edit')" size="lg">
    <div class="@container">
      <!-- Keyed by the transaction: every opening loads it afresh. -->
      <TransactionForm
        v-if="open"
        ref="form"
        :store-id="storeId"
        :transaction="props.transaction"
        @saved="open = false"
      />
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="transaction-form" type="submit" :disabled="!form?.valid || form?.pending">
        {{ t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
