<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { CurrencySelect } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { useSettingsStore } from '@/shared/store/settings'

import { updateInputOf, useStoreUpdate } from '../model/use-store-update'
import type { StoreFormProps } from '../store-edit.types'

const props = defineProps<StoreFormProps>()
const { t } = useI18n()
const settings = useSettingsStore()

const CATEGORIES: PropertyCategory[] = ['flat', 'house', 'car', 'land', 'other']

const category = ref<PropertyCategory>('flat')
const address = ref('')
const area = ref('')
const purchasedAt = ref('')
const purchasePrice = ref('')
const currency = ref('')

watch(
  () => props.store,
  (store) => {
    const info = store.property
    category.value = info?.category ?? 'flat'
    address.value = info?.address ?? ''
    area.value = info?.area ?? ''
    purchasedAt.value = info?.purchased_at ?? ''
    purchasePrice.value = info?.purchase_price ?? ''
    currency.value = info?.currency || settings.baseCurrency.toLowerCase()
  },
  { immediate: true },
)

const save = useStoreUpdate()

function submit() {
  save.mutate(
    updateInputOf(props.store, {
      property: {
        category: category.value,
        address: address.value.trim(),
        area: area.value.trim() || null,
        purchased_at: purchasedAt.value || null,
        purchase_price: purchasePrice.value.trim() || null,
        currency: currency.value,
      },
    }),
  )
}
</script>

<template>
  <form class="@container grid gap-4" @submit.prevent="submit">
    <div class="grid gap-4 @md:grid-cols-2">
      <div class="grid gap-1.5">
        <Label for="property-category">{{ t('propertyCategory.title') }}</Label>
        <Select id="property-category" v-model="category">
          <option v-for="entry in CATEGORIES" :key="entry" :value="entry">{{ t(`propertyCategory.${entry}`) }}</option>
        </Select>
      </div>
      <div class="grid gap-1.5">
        <Label for="property-area">{{ t('property.area') }}</Label>
        <Input id="property-area" v-model="area" inputmode="decimal" />
      </div>
    </div>
    <div class="grid gap-1.5">
      <Label for="property-address">{{ t('property.address') }}</Label>
      <Input id="property-address" v-model="address" />
    </div>
    <div class="grid gap-4 @md:grid-cols-3">
      <div class="grid gap-1.5">
        <Label for="property-purchased">{{ t('property.purchasedAt') }}</Label>
        <Input id="property-purchased" v-model="purchasedAt" type="date" />
      </div>
      <div class="grid gap-1.5">
        <Label for="property-price">{{ t('property.purchasePrice') }}</Label>
        <Input id="property-price" v-model="purchasePrice" inputmode="decimal" />
      </div>
      <CurrencySelect v-model="currency" :label="t('form.currency')" />
    </div>
    <div>
      <Button type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
