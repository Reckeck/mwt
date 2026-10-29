<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { ME, canUseWallet, useSettleForm } from '@/entities/shared-event'
import { currencySymbol, formatMoney } from '@/shared/lib/format'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { ReadonlyField } from '@/shared/ui/readonly-field'

import type { SharedEventPayShareProps } from '../shared-event-pay-share.types'

/**
 * `Pay my share` in an event somebody else leads: the sum is my share still unpaid (it may be
 * paid in parts), the recipient is the organizer and cannot be changed, the wallet is mine.
 * It writes one debt return to the organizer's contact, tied to the event.
 */
const props = defineProps<SharedEventPayShareProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()

const organizer = computed(() => props.event.people.find((person) => person.is_organizer))
const mine = computed(() => props.event.people.find((person) => person.id === ME))
const withWallet = computed(() => (mine.value ? canUseWallet(props.event, mine.value) : false))
const organizerName = computed(() => organizer.value?.name ?? '')

const { amount, storeId, date, nodes, owed, currency, decimals, form, pending, save } = useSettleForm({
  event: () => props.event,
  person: () => mine.value,
  open,
  withWallet: () => withWallet.value,
  kind: () => 'paid',
  name: organizerName,
})

// money: plain — the description is a string prop of FormDialog
const description = computed(() =>
  t('sharedEvents.pay.share', { title: props.event.event.title, amount: formatMoney(owed.value, { currency: currency.value, decimals: decimals.value }) }),
)
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-pay-share"
    form-id="shared-pay-share-form"
    size="form"
    :title="t('sharedEvents.pay.title', { name: organizerName })"
    :description="description"
    :submit-label="t('sharedEvents.pay.submit')"
    :pending="pending"
    phone-submit-only
    @submit="form.submit(save, $event)"
  >
    <FormField
      for="shared-pay-amount"
      :label="t('sharedEvents.settle.amount')"
      required
      :error="form.error('amount')"
      :hint="t('sharedEvents.pay.amountHint')"
    >
      <AmountInput id="shared-pay-amount" v-model="amount">
        <template #suffix>{{ currencySymbol(currency) }}</template>
      </AmountInput>
    </FormField>
    <FormField
      v-if="withWallet"
      for="shared-pay-wallet"
      :label="t('sharedEvents.settle.wallet')"
      required
      :error="form.error('store')"
    >
      <StoreNodeSelect id="shared-pay-wallet" v-model="storeId" :nodes="nodes" :invalid="Boolean(form.error('store'))" />
    </FormField>
    <FormField :hint="t('sharedEvents.pay.toHint')">
      <ReadonlyField :label="t('sharedEvents.pay.to')" :value="organizerName" />
    </FormField>
    <FormField for="shared-pay-date" :label="t('sharedEvents.settle.date')">
      <DateTimePicker id="shared-pay-date" v-model="date" mode="date" required />
    </FormField>
    <p class="text-xs text-muted-foreground">
      {{ withWallet ? t('sharedEvents.pay.note', { name: organizerName }) : t('sharedEvents.settle.offsetNote') }}
    </p>
  </FormDialog>
</template>
