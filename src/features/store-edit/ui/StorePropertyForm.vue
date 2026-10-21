<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencyPicker } from '@/entities/currency'
import { isHome, PROPERTY_CATEGORIES, shownPropertyCategory } from '@/entities/store'
import { useFormValidation } from '@/shared/lib/form'
import { Button } from '@/shared/ui/button'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { useSettingsStore } from '@/shared/store/settings'

import { type CarField, carDraftOf, carErrors, carInfoOf } from '../model/car-draft'
import { updateInputOf, useStoreUpdate } from '../model/use-store-update'
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

watch(
  () => props.store,
  (store) => {
    const info = store.property
    category.value = shownPropertyCategory(info?.category)
    address.value = info?.address ?? ''
    area.value = info?.area ?? ''
    currency.value = info?.currency || settings.baseCurrency.toLowerCase()
    payDay.value = info?.utilities_pay_day ? String(info.utilities_pay_day) : ''
    car.value = carDraftOf(info?.car)
  },
  { immediate: true },
)

const save = useStoreUpdate()

function submit() {
  save.mutate(
    updateInputOf(props.store, {
      property: {
        category: category.value,
        address: located.value ? address.value.trim() : '',
        area: located.value ? area.value.trim() || null : null,
        // the purchase now comes from the «Приобрести» operation; these stay as a fallback
        purchased_at: props.store.property?.purchased_at ?? null,
        purchase_price: props.store.property?.purchase_price ?? null,
        currency: currency.value,
        utilities_pay_day:
          home.value && payDay.value ? Math.min(31, Math.max(1, Number(payDay.value))) : undefined,
        car: isCar.value ? carInfoOf(car.value, props.store.property?.car) : undefined,
      },
    }),
  )
}
</script>

<template>
  <form class="@container grid gap-4" @submit.prevent="form.submit(submit, $event)">
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
      <Button type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
