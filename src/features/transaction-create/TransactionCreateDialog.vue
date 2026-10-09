<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import AddTransactionForm from './AddTransactionForm.vue'
import type { TransactionCreateDialogProps } from './add-transaction-form.types'

const props = withDefaults(defineProps<TransactionCreateDialogProps>(), { plan: null, disabled: false })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const form = ref<InstanceType<typeof AddTransactionForm> | null>(null)
</script>

<template>
  <Dialog v-model:open="open" :title="t('wallet.operation')" size="lg">
    <!-- The form's field rows are container queries; the dialog is outside the page container. -->
    <div class="@container">
      <AddTransactionForm
        ref="form"
        :store-id="props.storeId"
        :disabled="props.disabled"
        :plan="props.plan"
        @saved="open = false"
      />
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button
        form="transaction-form"
        type="submit"
        :disabled="props.disabled || !form?.valid || form?.pending"
      >
        {{ t('form.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
