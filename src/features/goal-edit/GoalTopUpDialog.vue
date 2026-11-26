<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { positiveAmount, useFormValidation } from '@/shared/lib/form'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'

/**
 * `Put aside`: put a sum (goal currency) aside for the goal in its wallet. The core caps it at the wallet's
 * free balance and answers with what was taken, so the toast names the real amount.
 */
const props = defineProps<{ goal: Goal; storeId: string }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()
const amount = ref('')
const form = useFormValidation<'amount'>(() => ({ amount: positiveAmount(amount.value) }), { resetOn: [open] })
watch(open, (isOpen) => {
  if (isOpen) amount.value = ''
})

const save = useMutation({
  mutationFn: () =>
    call<string>('fund_goal', { input: { goal_id: props.goal.id, store_id: props.storeId, amount: amount.value.trim() } }),
  onSuccess: () => {
    for (const queryKey of [...PLANNING_KEYS, ['envelopes']]) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'goalCard.topUpFailed', success: 'goalCard.topUpDone' },
})
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="goal-top-up"
    cancel-variant="outline"
    form-id="goal-top-up-form"
    :title="t('goalCard.topUpTitle', { name: props.goal.name })"
    :submit-label="t('goalCard.topUp')"
    :pending="save.isPending.value"
    size="sm"
    @submit="form.submit(save.mutate, $event)"
  >
    <FormField for="goal-top-up-amount" :label="t('goalCard.topUpAmount', { currency: props.goal.currency.toUpperCase() })" :error="form.error('amount')">
      <AmountInput id="goal-top-up-amount" v-model="amount" size="lg" :decimals="2" />
    </FormField>
  </FormDialog>
</template>
