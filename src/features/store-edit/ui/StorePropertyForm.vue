<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { isHome, PROPERTY_CATEGORIES, shownPropertyCategory } from '@/entities/store'
import { useFormValidation } from '@/shared/lib/form'
import { Button } from '@/shared/ui/button'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { SyncChangedBanner } from '@/shared/ui/sync-changed-banner'
import { useSettingsStore } from '@/shared/store/settings'

import { type CarDraft, type CarField, carDraftOf, carErrors, carInfoOf } from '../model/car-draft'
import { useStoreFormEdit } from '../model/use-store-form-edit'
import type { StoreFormProps } from '../store-edit.types'
import { CarFields } from './car-fields'

const props = defineProps<StoreFormProps>()
const { t } = useI18n()
const settings = useSettingsStore()

const category = ref<PropertyCategory>('flat')
const address = ref('')
const area = ref('')
const currency = ref('')
/** Day of the month all utilities are paid; empty — no fixed day. */
const payDay = ref('')
const car = ref(carDraftOf(undefined))

const home = computed(() => isHome(category.value))
const isCar = computed(() => category.value === 'car')
/** A home and a plot have an address and an area; a car and «other» do not. */
const located = computed(() => home.value || category.value === 'land')

const form = useFormValidation<CarField>(() => (isCar.value ? carErrors(car.value) : {}), { resetOn: () => props.store.id })

interface PropertyFields {
  category: PropertyCategory
  address: string
  area: string
  currency: string
  payDay: string
  car: CarDraft
}
const edit = useStoreFormEdit<PropertyFields>({
  store: () => props.store,
  model: (store) => {
    const info = store.property
    return {
      category: shownPropertyCategory(info?.category),
      address: info?.address ?? '',
      area: info?.area ?? '',
      currency: info?.currency || settings.baseCurrency.toLowerCase(),
      payDay: info?.utilities_pay_day ? String(info.utilities_pay_day) : '',
      car: carDraftOf(info?.car),
    }
  },
  current: () => ({
    category: category.value,
    address: address.value,
    area: area.value,
    currency: currency.value,
    payDay: payDay.value,
    car: { ...car.value },
  }),
  load: (fields) => {
    category.value = fields.category
    address.value = fields.address
    area.value = fields.area
    currency.value = fields.currency
    payDay.value = fields.payDay
    car.value = fields.car
  },
})

function submit() {
  edit.save({
    property: {
        category: category.value,
        address: located.value ? address.value.trim() : '',
        area: located.value ? area.value.trim() || null : null,
        // the purchase now comes from the `Acquire` operation; these stay as a fallback
        purchased_at: props.store.property?.purchased_at ?? null,
        purchase_price: props.store.property?.purchase_price ?? null,
        currency: currency.value,
        utilities_pay_day:
          home.value && payDay.value ? Math.min(31, Math.max(1, Number(payDay.value))) : undefined,
        car: isCar.value ? carInfoOf(car.value, props.store.property?.car) : undefined,
    },
  })
}
</script>

<template>
  <form class="@container grid gap-4" @submit.prevent="form.submit(submit, $event)">
    <SyncChangedBanner v-if="edit.changedElsewhere.value" @refresh="edit.refreshFromOtherDevice" />
    <div class="grid gap-4 @mid:grid-cols-2">
      <div class="grid gap-1.5" data-tour="property-category">
        <Label for="property-category">{{ t('propertyCategory.title') }}</Label>
        <Select id="property-category" v-model="category">
          <SelectItem v-for="entry in PROPERTY_CATEGORIES" :key="entry" :value="entry">{{ t(`propertyCategory.${entry}`) }}</SelectItem>
        </Select>
      </div>
      <div v-if="located" class="grid gap-1.5">
        <Label for="property-area">{{ t(home ? 'property.area' : 'property.areaSotki') }}</Label>
        <AmountInput id="property-area" v-model="area" />
      </div>
    </div>
    <div v-if="located" class="grid gap-1.5">
      <Label for="property-address">{{ t('property.address') }}</Label>
      <Input id="property-address" v-model="address" />
    </div>
    <div class="grid gap-4 @mid:w-1/3">
      <CurrencyPicker v-model="currency" :label="t('form.currency')" />
    </div>
    <CarFields v-if="isCar" v-model="car" :error="form.error" />
    <div v-if="home" class="grid gap-1.5 @mid:w-1/3">
      <Label for="property-pay-day">{{ t('utilities.payDay') }}</Label>
      <Input id="property-pay-day" v-model="payDay" type="text" inputmode="numeric" min="1" max="31" :placeholder="t('utilities.payDayHint')" />
    </div>
    <div>
      <Button type="submit" :disabled="edit.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
