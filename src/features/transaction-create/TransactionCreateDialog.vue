<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

import TransactionForm from './TransactionForm.vue'
import type { TransactionCreateDialogProps } from './transaction-form.types'

const props = withDefaults(defineProps<TransactionCreateDialogProps>(), {
  plan: null,
  disabled: false,
  counterpart: null,
  flowId: null,
  prefill: null,
})
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const form = ref<InstanceType<typeof TransactionForm> | null>(null)
</script>

<template>
  <Dialog v-model:open="open" :title="t('wallet.operation')" size="lg">
    <!-- The form's field rows are container queries; the dialog is outside the page container. -->
    <div class="@container">
      <TransactionForm
        v-if="open"
        ref="form"
        :store-id="props.storeId"
        :disabled="props.disabled"
        :plan="props.plan"
        :counterpart="props.counterpart"
        :flow-id="props.flowId"
        :prefill="props.prefill"
        @saved="open = false"
      />
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="transaction-form" type="submit" :disabled="!form?.valid || form?.pending">
        {{ t('form.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
