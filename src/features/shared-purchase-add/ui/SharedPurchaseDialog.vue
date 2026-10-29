<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { ME, sumDecimals } from '@/entities/shared-event'
import { formatMoney } from '@/shared/lib/format'
import { AmountInput } from '@/shared/ui/amount-input'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { usePurchaseForm } from '../model/use-purchase-form'
import type { SharedPurchaseDialogProps } from '../shared-purchase-add.types'

/**
 * «Купили · Пиво»: who bought, how many, for how much, in which currency; a purchase of mine also
 * says which wallet the payment comes from (it becomes a real payment), a foreign currency
 * offers a rate. A sheet from below on a phone.
 */
const props = defineProps<SharedPurchaseDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const draft = usePurchaseForm(open, props)

const buyers = computed(() =>
  props.event.people
    .filter((person) => (person.id === ME ? props.event.event.i_participate : true))
    .map((person) => ({ id: person.id, name: person.name || t('sharedEvents.me') })),
)
/** The item the dialog is about: the planned one, or the one the changed purchase belongs to. */
const target = computed(
  () =>
    props.item ??
    props.event.event.items.find((entry) => entry.purchases.some((purchase) => purchase.id === props.purchase?.id)) ??
    null,
)
const unit = computed(() => (draft.isNew.value ? draft.unit.value : (target.value?.unit ?? '')))
const heading = computed(() => target.value?.title ?? '')
const title = computed(() => {
  if (props.purchase) return t('sharedEvents.purchase.changeTitle', { title: heading.value })
  return props.item ? t('sharedEvents.purchase.title', { title: heading.value }) : t('sharedEvents.purchase.unplannedTitle')
})
// money: plain — a text line under the title
const planLine = computed(() => {
  const plan = props.item?.plan
  const view = props.event.items.find((entry) => entry.item_id === props.item?.id)
  if (!plan || !view?.plan_total) return ''
  return t('sharedEvents.purchase.plan', {
    qty: plan.qty,
    unit: props.item?.unit ?? '',
    total: formatMoney(view.plan_total, { currency: props.event.event.currency, decimals: sumDecimals(props.event) }),
  })
})
// money: plain — a hint line
const unitHint = computed(() =>
  draft.unitPriceHint.value
    ? t('sharedEvents.purchase.unitPrice', {
        unit: unit.value,
        price: formatMoney(draft.unitPriceHint.value, { currency: draft.currency.value, decimals: sumDecimals(props.event) }),
      })
    : undefined,
)
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-purchase"
    form-id="shared-purchase-form"
    size="sm"
    :title="title"
    :description="planLine || undefined"
    :pending="draft.pending.value"
    @submit="draft.form.submit(draft.save, $event)"
  >
    <FormField v-if="draft.isNew.value" for="purchase-title" :label="t('sharedEvents.item.title')" required :error="draft.form.error('title')">
      <Input id="purchase-title" v-model="draft.title.value" autocomplete="off" :placeholder="t('sharedEvents.item.titlePlaceholder')" />
    </FormField>

    <FormField :label="t('sharedEvents.purchase.buyer')" required :error="draft.form.error('buyer')">
      <ToggleGroup
        type="single"
        variant="outline"
        :spacing="2"
        class="flex-wrap"
        :model-value="draft.buyer.value"
        :disabled="draft.linked.value"
        :aria-label="t('sharedEvents.purchase.buyer')"
        @update:model-value="(value) => value && (draft.buyer.value = String(value))"
      >
        <ToggleGroupItem v-for="person in buyers" :key="person.id" :value="person.id" class="px-3">{{ person.name }}</ToggleGroupItem>
      </ToggleGroup>
    </FormField>

    <FormField for="purchase-qty" :label="t('sharedEvents.purchase.qty')" required :error="draft.form.error('qty')" :hint="unitHint">
      <AmountInput id="purchase-qty" v-model="draft.qty.value" :calculator="false">
        <template #suffix><span class="text-sm text-muted-foreground">{{ unit }}</span></template>
      </AmountInput>
    </FormField>

    <div class="grid gap-4 @xs:grid-cols-2 [&>*]:min-w-0">
      <FormField for="purchase-amount" :label="t('sharedEvents.purchase.amount')" required :error="draft.form.error('amount')">
        <AmountInput id="purchase-amount" v-model="draft.amount.value" :decimals="2" :disabled="draft.linked.value" />
      </FormField>
      <CurrencyPicker
        v-model="draft.currency.value"
        :label="t('sharedEvents.purchase.currency')"
        required
        :disabled="draft.linked.value"
        :error="draft.form.error('currency')"
      />
    </div>

    <FormField
      v-if="draft.foreign.value && !draft.linked.value"
      for="purchase-rate"
      :label="t('sharedEvents.purchase.rate', { from: draft.currency.value.toUpperCase(), to: props.event.event.currency.toUpperCase() })"
      :error="draft.form.error('rate')"
      :hint="t('sharedEvents.purchase.rateHint')"
    >
      <AmountInput id="purchase-rate" v-model="draft.rate.value" :calculator="false" />
    </FormField>

    <FormField
      v-if="draft.mine.value && !draft.linked.value"
      :label="t('sharedEvents.purchase.wallet')"
      required
      :error="draft.form.error('store')"
      :hint="t('sharedEvents.purchase.walletHint')"
    >
      <StoreNodeSelect id="purchase-wallet" v-model="draft.wallet.storeId.value" :nodes="draft.wallet.nodes.value" :invalid="Boolean(draft.form.error('store'))" />
    </FormField>
    <p v-if="draft.linked.value" class="text-xs text-muted-foreground">{{ t('sharedEvents.purchase.linkedNote') }}</p>

    <FormField for="purchase-date" :label="t('sharedEvents.purchase.date')">
      <DateTimePicker id="purchase-date" v-model="draft.date.value" mode="date" required />
    </FormField>
  </FormDialog>
</template>
