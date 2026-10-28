<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { ContactPicker } from '@/entities/contact'
import { useCategories, useContacts } from '@/shared/api/queries'
import { categoryOptions } from '@/shared/lib/categories'
import { Combobox } from '@/shared/ui/combobox'
import { Label } from '@/shared/ui/label'

import { AmountField } from '../amount-field'
import { PeriodField } from '../period-field'
import type { FilterFieldProps } from '../../transaction-filter.types'

/** The folded filters, in the panel's order: period, category, contact, amount. */
const props = defineProps<FilterFieldProps>()
const filter = defineModel<TxFilter>({ required: true })
/** Fields that go right after the period (a phone's wallet and kind). */
defineSlots<{ default?(): unknown }>()

const { t } = useI18n()
const contacts = useContacts()
const categories = useCategories()
const options = computed(() => categoryOptions(categories.data.value, t))

const category = computed({
  get: () => filter.value.category ?? '',
  set: (value: string | string[]) => {
    const key = Array.isArray(value) ? (value[0] ?? '') : value
    filter.value = { ...filter.value, category: key || null }
  },
})
const contactId = computed({
  get: () => filter.value.contact_id ?? '',
  set: (value: string) => {
    filter.value = { ...filter.value, contact_id: value || null }
  },
})
</script>

<template>
  <PeriodField v-model="filter" :id-prefix="props.idPrefix" />
  <slot />
  <div class="grid min-w-0 content-start gap-1.5">
    <Label :for="`${props.idPrefix}-category`">{{ t('filter.category') }}</Label>
    <Combobox
      :id="`${props.idPrefix}-category`"
      v-model="category"
      :options="options"
      :placeholder="t('filter.allCategories')"
      :empty-label="t('categorySelect.empty')"
    />
  </div>
  <div class="grid min-w-0 content-start gap-1.5">
    <Label :for="`${props.idPrefix}-contact`">{{ t('filter.contact') }}</Label>
    <ContactPicker
      :id="`${props.idPrefix}-contact`"
      v-model="contactId"
      no-create
      :empty-label="t('filter.allContacts')"
      :disabled="!contacts.data.value?.length"
    />
  </div>
  <AmountField v-model="filter" :id-prefix="props.idPrefix" />
</template>
