<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { ME, useSettleForm } from '@/entities/shared-event'
import { currencySymbol, formatMoney } from '@/shared/lib/format'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'

import type { SharedSettleDialogProps } from '../shared-settle.types'

/**
 * «Получил» / «Отдал» / «Зачесть без операции» for one person of an event: the sum starts at what
 * is owed and may be smaller (the rest stays a debt), the wallet is mine, the date is today.
 * The save toast offers «Отменить».
 */
const props = defineProps<SharedSettleDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()

/** Money with me and an organizer who is not me is with the organizer: «Получил от Ани». */
const name = computed(() => {
  const person = props.person
  const organizer = props.event.people.find((entry) => entry.is_organizer)
  if (person?.id === ME && props.kind !== 'offset') return organizer?.name ?? ''
  return person?.name || t('sharedEvents.me')
})
const { amount, storeId, date, nodes, owed, currency, decimals, form, pending, save } = useSettleForm({
  event: () => props.event,
  person: () => props.person ?? undefined,
  open,
  withWallet: () => props.withWallet,
  kind: () => props.kind,
  name,
})

const title = computed(() => t(`sharedEvents.settle.title.${props.kind}`, { name: name.value }))
// money: plain — the description is a string prop of FormDialog
const description = computed(() =>
  t('sharedEvents.settle.owed', { title: props.event.event.title, amount: formatMoney(owed.value, { currency: currency.value, decimals: decimals.value }) }),
)
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-settle"
    form-id="shared-settle-form"
    size="form"
    :title="title"
    :description="description"
    :submit-label="props.kind === 'offset' ? t('sharedEvents.settle.submitOffset') : t('sharedEvents.settle.submit')"
    :pending="pending"
    phone-submit-only
    @submit="form.submit(save, $event)"
  >
    <FormField
      for="shared-settle-amount"
      :label="t('sharedEvents.settle.amount')"
      required
      :error="form.error('amount')"
      :hint="t('sharedEvents.settle.amountHint')"
    >
      <AmountInput id="shared-settle-amount" v-model="amount">
        <template #suffix>{{ currencySymbol(currency) }}</template>
      </AmountInput>
    </FormField>
    <FormField
      v-if="props.withWallet"
      for="shared-settle-wallet"
      :label="t('sharedEvents.settle.wallet')"
      required
      :error="form.error('store')"
    >
      <StoreNodeSelect id="shared-settle-wallet" v-model="storeId" :nodes="nodes" :invalid="Boolean(form.error('store'))" />
    </FormField>
    <FormField for="shared-settle-date" :label="t('sharedEvents.settle.date')">
      <DateTimePicker id="shared-settle-date" v-model="date" mode="date" required />
    </FormField>
    <p v-if="!props.withWallet" class="text-xs text-muted-foreground">{{ t('sharedEvents.settle.offsetNote') }}</p>
  </FormDialog>
</template>
