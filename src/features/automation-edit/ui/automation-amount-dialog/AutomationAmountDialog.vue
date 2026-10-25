<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { call } from '@/shared/api/ipc'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

import type { AutomationAmountDialogProps } from './automation-amount-dialog.types'

/**
 * «Сколько пришло»: the real amount of an operation an automation booked
 * approximately (a salary that differs every time). It replaces the estimate
 * and makes the next one.
 */
const props = withDefaults(defineProps<AutomationAmountDialogProps>(), { estimate: '', symbol: '' })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()
const amount = ref('')

watch(open, (isOpen) => {
  if (isOpen) amount.value = props.estimate
})

const form = useFormValidation(() => ({ amount: positiveAmount(amount.value) }), { resetOn: open })

const save = useMutation({
  mutationFn: () => call<Transaction>('confirm_estimated_amount', { input: { tx_id: props.txId, amount: amount.value } }),
  onSuccess: () => {
    for (const queryKey of [...PLANNING_KEYS, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'automation.amount.failed', success: 'automation.amount.saved' },
})
</script>

<template>
  <FormDialog overlay="automation-amount"
    v-model:open="open"
    :title="t('automation.amount.title', { name: props.name })"
    :description="t('automation.amount.description')"
    size="sm"
    form-id="automation-amount-form"
    :pending="save.isPending.value"
    @submit="form.submit(save.mutate, $event)"
  >
    <FormField
      for="automation-amount"
      :label="t('automation.amount.label')"
      required
      :error="form.error('amount')"
    >
      <template v-if="props.estimate" #hint>
        <i18n-t keypath="automation.amount.hint">
          <template #estimate><Money :amount="props.estimate" :currency="props.symbol" /></template>
        </i18n-t>
      </template>
      <AmountInput id="automation-amount" v-model="amount">
        <template v-if="props.symbol" #suffix>{{ props.symbol }}</template>
      </AmountInput>
    </FormField>
  </FormDialog>
</template>
