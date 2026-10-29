<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useAddSharedItem, useUpdateSharedItem } from '@/entities/shared-event'
import { decimalMul, parseDecimal } from '@/shared/lib/decimal'
import { firstError, positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { ITEM_UNITS } from '../shared-item-edit.constants'
import type { SharedItemEditDialogProps } from '../shared-item-edit.types'

/**
 * «Добавить позицию» and «Изменить»: a name, a unit and an optional plan — how many and the price
 * of one or of all. Without a plan the item is just a name that gets bought later.
 */
const props = defineProps<SharedItemEditDialogProps>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const title = ref('')
const unit = ref('шт')
const qty = ref('')
const price = ref('')
const mode = ref<PriceMode>('per_unit')

watch(open, (now) => {
  if (!now) return
  const plan = props.item?.plan
  title.value = props.item?.title ?? ''
  unit.value = props.item?.unit ?? 'шт'
  qty.value = plan?.qty ?? ''
  price.value = plan?.price ?? ''
  mode.value = plan?.price_mode ?? 'per_unit'
})

const hasPlan = computed(() => Boolean(qty.value.trim() || price.value.trim()))
const form = useFormValidation(
  () => ({
    title: required(title.value),
    qty: hasPlan.value ? positiveAmount(qty.value) : null,
    price: hasPlan.value ? firstError(required(price.value), positiveAmount(price.value)) : null,
  }),
  { resetOn: open },
)

const total = computed(() => {
  const count = parseDecimal(qty.value)
  const sum = parseDecimal(price.value)
  if (count === null || sum === null) return null
  return mode.value === 'total' ? sum : decimalMul(count, sum)
})

const add = useAddSharedItem()
const change = useUpdateSharedItem()
const pending = computed(() => add.isPending.value || change.isPending.value)

function save() {
  const draft: SharedItemDraft = {
    title: title.value.trim(),
    unit: unit.value.trim() || 'шт',
    category_id: props.item?.category_id ?? null,
    plan: hasPlan.value
      ? { qty: parseDecimal(qty.value) ?? qty.value, price: parseDecimal(price.value) ?? price.value, price_mode: mode.value }
      : null,
    cancelled: props.item?.cancelled ?? false,
  }
  const close = { onSuccess: () => (open.value = false) }
  if (props.item) change.mutate({ event_id: props.event.event.id, item_id: props.item.id, item: draft }, close)
  else add.mutate({ event_id: props.event.event.id, item: draft }, close)
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    overlay="shared-item-edit"
    form-id="shared-item-form"
    size="sm"
    :title="props.item ? t('sharedEvents.item.change') : t('sharedEvents.item.add')"
    :description="t('sharedEvents.item.description')"
    :submit-label="props.item ? t('actions.save') : t('actions.add')"
    :pending="pending"
    @submit="form.submit(save, $event)"
  >
    <FormField for="item-title" :label="t('sharedEvents.item.title')" required :error="form.error('title')">
      <Input id="item-title" v-model="title" autocomplete="off" :placeholder="t('sharedEvents.item.titlePlaceholder')" />
    </FormField>
    <div class="grid gap-4 @xs:grid-cols-2 [&>*]:min-w-0">
      <FormField for="item-qty" :label="t('sharedEvents.item.qty')" :error="form.error('qty')" :hint="t('sharedEvents.item.planHint')">
        <AmountInput id="item-qty" v-model="qty" :calculator="false" />
      </FormField>
      <FormField for="item-unit" :label="t('sharedEvents.item.unit')">
        <Input id="item-unit" v-model="unit" list="item-units" autocomplete="off" />
        <datalist id="item-units"><option v-for="entry in ITEM_UNITS" :key="entry" :value="entry" /></datalist>
      </FormField>
    </div>
    <FormField for="item-price" :label="t('sharedEvents.item.price')" :error="form.error('price')">
      <AmountInput id="item-price" v-model="price" :decimals="2" />
    </FormField>
    <div class="flex flex-wrap items-center gap-3">
      <ToggleGroup
        type="single"
        variant="outline"
        size="sm"
        :model-value="mode"
        :aria-label="t('sharedEvents.item.priceMode')"
        @update:model-value="(value) => value && (mode = value as PriceMode)"
      >
        <ToggleGroupItem value="per_unit">{{ t('sharedEvents.item.perUnit') }}</ToggleGroupItem>
        <ToggleGroupItem value="total">{{ t('sharedEvents.item.total') }}</ToggleGroupItem>
      </ToggleGroup>
      <span v-if="total" class="text-sm text-muted-foreground">
        {{ t('sharedEvents.item.planTotal') }}
        <Money :amount="total" :currency="props.event.event.currency" :convert="false" class="font-medium text-foreground" />
      </span>
    </div>
  </FormDialog>
</template>
